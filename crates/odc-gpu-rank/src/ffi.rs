//! The helper's `unsafe` code which, with the generated `bindings`, is the only such code in
//! OpenDeepCool (Phase 6 plan, Task 9): the three DRM requests that tell a GPU's kind, and
//! NVIDIA's CUDA library.
//!
//! - The request numbers are built from the generated structures and constants the way the
//!   kernel headers' `DRM_IOW` and `DRM_IOWR` build them, with `libc`'s `_IOW` and `_IOWR`.
//! - CUDA's five functions are declared by hand from NVIDIA's CUDA Driver API documentation (read
//!   2026-10-29). Their header comes with NVIDIA's developer toolkit, not with the driver
//!   (unverified), so a normal PC has nothing to generate them from.

#![allow(unsafe_code)]

use std::ffi::{CStr, c_char, c_int, c_uint};
use std::fs::File;
use std::io;
use std::marker::PhantomData;
use std::os::fd::AsRawFd;
use std::ptr;

use libloading::Library;

use crate::bindings::{
    AMDGPU_INFO_DEV_INFO, DRM_AMDGPU_INFO, DRM_COMMAND_BASE, DRM_I915_QUERY,
    DRM_I915_QUERY_MEMORY_REGIONS, DRM_IOCTL_BASE, DRM_XE_DEVICE_QUERY, DRM_XE_DEVICE_QUERY_CONFIG,
    drm_amdgpu_info, drm_amdgpu_info_device, drm_i915_query, drm_i915_query_item,
    drm_xe_device_query,
};

/// The most a query's answer may take. The kernel's answers here take a few hundred bytes.
const MAX_ANSWER: usize = 64 * 1024;

/// A DRM request and the structure it carries, so a call can't pass another.
pub(crate) struct Request<T> {
    pub(crate) number: libc::Ioctl,
    arg: PhantomData<fn(&mut T)>,
}

impl<T> Request<T> {
    /// `DRM_IOW(DRM_COMMAND_BASE + nr, T)` (`drm.h`): the kernel reads a `T`.
    const fn write(nr: u32) -> Self {
        Request {
            number: libc::_IOW::<T>(DRM_IOCTL_BASE as u32, DRM_COMMAND_BASE + nr),
            arg: PhantomData,
        }
    }

    /// `DRM_IOWR(DRM_COMMAND_BASE + nr, T)` (`drm.h`): the kernel reads a `T` and writes it back.
    const fn read_write(nr: u32) -> Self {
        Request {
            number: libc::_IOWR::<T>(DRM_IOCTL_BASE as u32, DRM_COMMAND_BASE + nr),
            arg: PhantomData,
        }
    }
}

/// `DRM_IOCTL_AMDGPU_INFO` (`amdgpu_drm.h`).
pub(crate) const AMDGPU_INFO: Request<drm_amdgpu_info> = Request::write(DRM_AMDGPU_INFO);
/// `DRM_IOCTL_XE_DEVICE_QUERY` (`xe_drm.h`).
pub(crate) const XE_DEVICE_QUERY: Request<drm_xe_device_query> =
    Request::read_write(DRM_XE_DEVICE_QUERY);
/// `DRM_IOCTL_I915_QUERY` (`i915_drm.h`).
pub(crate) const I915_QUERY: Request<drm_i915_query> = Request::read_write(DRM_I915_QUERY);

/// Sends `request` with `arg` to the DRM device `node`, again while the call is interrupted or
/// asked to retry, as libdrm's `drmIoctl` does.
///
/// # Safety
///
/// Every address inside `arg` must point to memory the kernel may write, as much of it as `arg`
/// tells the kernel to, for the call's duration.
unsafe fn ioctl<T>(node: &File, request: &Request<T>, arg: &mut T) -> io::Result<()> {
    loop {
        // SAFETY: `node` is an open file, and `request` was built from `T`'s size, so the kernel
        // reads and writes at most that much of `arg`, which is ours for the call. The caller
        // vouches for the addresses inside it.
        let result = unsafe { libc::ioctl(node.as_raw_fd(), request.number, ptr::from_mut(arg)) };
        if result != -1 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if !matches!(error.raw_os_error(), Some(libc::EINTR | libc::EAGAIN)) {
            return Err(error);
        }
    }
}

/// The address the kernel writes an answer to.
fn address<T: ?Sized>(place: &mut T) -> u64 {
    ptr::from_mut(place).cast::<u8>().expose_provenance() as u64
}

/// A zeroed buffer for an answer of `len` bytes, the size the kernel asked for.
fn buffer(len: usize) -> io::Result<Vec<u8>> {
    if len > MAX_ANSWER {
        return Err(io::Error::other(format!(
            "the answer would take {len} bytes, more than {MAX_ANSWER}"
        )));
    }
    Ok(vec![0; len])
}

/// amdgpu's device information (`AMDGPU_INFO_DEV_INFO`), which holds `ids_flags`.
pub(crate) fn amdgpu_device_info(node: &File) -> io::Result<drm_amdgpu_info_device> {
    let mut info = drm_amdgpu_info_device::default();
    let mut request = drm_amdgpu_info {
        return_pointer: address(&mut info),
        return_size: size_of::<drm_amdgpu_info_device>() as u32,
        query: AMDGPU_INFO_DEV_INFO,
        ..Default::default()
    };
    // SAFETY: the kernel writes at most `return_size` bytes, the size of `info`, at
    // `return_pointer`, which is `info`.
    unsafe { ioctl(node, &AMDGPU_INFO, &mut request) }?;
    Ok(info)
}

/// xe's configuration (`DRM_XE_DEVICE_QUERY_CONFIG`): a `drm_xe_query_config` and its parameters,
/// as the kernel wrote them. Asked twice: first for its size, then for the data.
pub(crate) fn xe_query_config(node: &File) -> io::Result<Vec<u8>> {
    let mut query = drm_xe_device_query {
        query: DRM_XE_DEVICE_QUERY_CONFIG,
        ..Default::default()
    };
    // SAFETY: with `size` 0 and no `data`, the kernel writes only the size the answer takes,
    // into `query` itself.
    unsafe { ioctl(node, &XE_DEVICE_QUERY, &mut query) }?;
    let mut answer = buffer(query.size as usize)?;
    query.data = address(answer.as_mut_slice());
    // SAFETY: the kernel writes `size` bytes at `data`, which is `answer`, of exactly that
    // length; it refuses any other size.
    unsafe { ioctl(node, &XE_DEVICE_QUERY, &mut query) }?;
    Ok(answer)
}

/// i915's memory regions (`DRM_I915_QUERY_MEMORY_REGIONS`): a `drm_i915_query_memory_regions`
/// and its regions, as the kernel wrote them. Asked twice: first for its length, then for the
/// data.
pub(crate) fn i915_memory_regions(node: &File) -> io::Result<Vec<u8>> {
    let mut item = drm_i915_query_item {
        query_id: u64::from(DRM_I915_QUERY_MEMORY_REGIONS),
        ..Default::default()
    };
    let mut query = drm_i915_query {
        num_items: 1,
        items_ptr: address(&mut item),
        ..Default::default()
    };
    // SAFETY: `items_ptr` is `item`, the one item `num_items` gives. With `length` 0 and no
    // `data_ptr`, the kernel writes only the length the answer takes, into `item`.
    unsafe { ioctl(node, &I915_QUERY, &mut query) }?;
    let mut answer = buffer(item_length(item.length)?)?;
    item.data_ptr = address(answer.as_mut_slice());
    query.items_ptr = address(&mut item);
    // SAFETY: `items_ptr` is `item` again, and the kernel writes at most `length` bytes at
    // `data_ptr`, which is `answer`, of that length.
    unsafe { ioctl(node, &I915_QUERY, &mut query) }?;
    answer.truncate(item_length(item.length)?);
    Ok(answer)
}

/// A query item's length, or the error the kernel gave for that item as a negative errno
/// (`i915_drm.h`, `drm_i915_query_item`).
fn item_length(length: i32) -> io::Result<usize> {
    usize::try_from(length)
        .map_err(|_| io::Error::from_raw_os_error(length.checked_neg().unwrap_or(libc::EINVAL)))
}

/// NVIDIA's CUDA library, as NVIDIA's driver installs it.
const CUDA_LIBRARY: &str = "libcuda.so.1";

/// `CUresult`, a C enum; `CUDA_SUCCESS` is 0 (CUDA Driver API, data types).
type CuResult = c_int;

/// `CUdevice`: `typedef int CUdevice_v1` (CUDA Driver API, data types).
pub(crate) type CuDevice = c_int;

const CUDA_SUCCESS: CuResult = 0;

/// `CU_DEVICE_ATTRIBUTE_INTEGRATED`, 18: "Device is integrated with host memory" (CUDA Driver
/// API, `CUdevice_attribute`). The enum is passed as a C `int`.
const CU_DEVICE_ATTRIBUTE_INTEGRATED: c_int = 18;

type CuInit = unsafe extern "C" fn(flags: c_uint) -> CuResult;
type CuDeviceGetCount = unsafe extern "C" fn(count: *mut c_int) -> CuResult;
type CuDeviceGet = unsafe extern "C" fn(device: *mut CuDevice, ordinal: c_int) -> CuResult;
type CuDeviceGetAttribute =
    unsafe extern "C" fn(pi: *mut c_int, attrib: c_int, dev: CuDevice) -> CuResult;
type CuDeviceGetPciBusId =
    unsafe extern "C" fn(pci_bus_id: *mut c_char, len: c_int, dev: CuDevice) -> CuResult;

/// NVIDIA's CUDA library, loaded and initialised.
pub(crate) struct Cuda {
    device_get_count: CuDeviceGetCount,
    device_get: CuDeviceGet,
    device_get_attribute: CuDeviceGetAttribute,
    device_get_pci_bus_id: CuDeviceGetPciBusId,
    /// Keeps the functions above loaded for as long as they can be called.
    _library: Library,
}

impl Cuda {
    /// Loads the library and calls `cuInit`. The error is the loader's or CUDA's.
    pub(crate) fn load() -> Result<Cuda, String> {
        // SAFETY: loading the library runs its initialisers. It's the library NVIDIA's driver
        // installs for programs to use, and it's loaded only for a GPU on that driver.
        let library = unsafe { Library::new(CUDA_LIBRARY) }.map_err(|e| e.to_string())?;
        // SAFETY: each type is the function's prototype in the CUDA Driver API documentation.
        let (init, device_get_count, device_get, device_get_attribute, device_get_pci_bus_id) = unsafe {
            (
                symbol::<CuInit>(&library, b"cuInit\0")?,
                symbol::<CuDeviceGetCount>(&library, b"cuDeviceGetCount\0")?,
                symbol::<CuDeviceGet>(&library, b"cuDeviceGet\0")?,
                symbol::<CuDeviceGetAttribute>(&library, b"cuDeviceGetAttribute\0")?,
                symbol::<CuDeviceGetPciBusId>(&library, b"cuDeviceGetPCIBusId\0")?,
            )
        };
        // SAFETY: "Currently, the Flags parameter must be 0" (CUDA Driver API, `cuInit`).
        check("cuInit", unsafe { init(0) })?;
        Ok(Cuda {
            device_get_count,
            device_get,
            device_get_attribute,
            device_get_pci_bus_id,
            _library: library,
        })
    }

    /// Every CUDA device, with its PCI bus ID as CUDA writes it: `[domain]:[bus]:[device].
    /// [function]`, all in hexadecimal (CUDA Driver API, `cuDeviceGetPCIBusId`).
    pub(crate) fn devices(&self) -> Result<Vec<(CuDevice, String)>, String> {
        let mut count: c_int = 0;
        // SAFETY: CUDA writes one `int` to `count`.
        check("cuDeviceGetCount", unsafe {
            (self.device_get_count)(&mut count)
        })?;
        (0..count)
            .map(|ordinal| {
                let mut device: CuDevice = 0;
                // SAFETY: CUDA writes one `CUdevice` to `device`; `ordinal` is below the count.
                check("cuDeviceGet", unsafe {
                    (self.device_get)(&mut device, ordinal)
                })?;
                // It "should be large enough to store 13 characters including the
                // NULL-terminator" (CUDA Driver API).
                let mut bus_id = [0u8; 32];
                // SAFETY: CUDA writes at most `len` bytes, the buffer's length, nul included.
                check("cuDeviceGetPCIBusId", unsafe {
                    (self.device_get_pci_bus_id)(
                        bus_id.as_mut_ptr().cast::<c_char>(),
                        bus_id.len() as c_int,
                        device,
                    )
                })?;
                let text = CStr::from_bytes_until_nul(&bus_id)
                    .map_err(|_| "cuDeviceGetPCIBusId wrote no ending nul".to_owned())?;
                Ok((device, text.to_string_lossy().into_owned()))
            })
            .collect()
    }

    /// Whether `device` is integrated: `CU_DEVICE_ATTRIBUTE_INTEGRATED` isn't 0.
    pub(crate) fn integrated(&self, device: CuDevice) -> Result<bool, String> {
        let mut value: c_int = 0;
        // SAFETY: CUDA writes one `int` to `value`; `device` came from `cuDeviceGet`.
        check("cuDeviceGetAttribute", unsafe {
            (self.device_get_attribute)(&mut value, CU_DEVICE_ATTRIBUTE_INTEGRATED, device)
        })?;
        Ok(value != 0)
    }
}

/// The function `name` in `library`, copied out of its symbol; [`Cuda`] keeps the library
/// loaded while it can be called.
///
/// # Safety
///
/// `T` must be the function's type.
unsafe fn symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T, String> {
    // SAFETY: the caller vouches for `T`.
    unsafe { library.get::<T>(name) }
        .map(|symbol| *symbol)
        .map_err(|e| e.to_string())
}

/// `Ok` for `CUDA_SUCCESS`, else the call and its error number.
fn check(call: &str, result: CuResult) -> Result<(), String> {
    if result == CUDA_SUCCESS {
        Ok(())
    } else {
        Err(format!("{call} failed with CUDA error {result}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The numbers the headers' own macros give on x86_64, printed by a C program built with
    /// clang against the installed headers on 2026-10-29: `DRM_IOCTL_AMDGPU_INFO`,
    /// `DRM_IOCTL_XE_DEVICE_QUERY` and `DRM_IOCTL_I915_QUERY`.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn the_requests_are_numbered_as_the_headers_number_them() {
        assert_eq!(AMDGPU_INFO.number, 0x4020_6445);
        assert_eq!(XE_DEVICE_QUERY.number, 0xc028_6440);
        assert_eq!(I915_QUERY.number, 0xc010_6479);
    }

    #[test]
    fn an_item_length_below_zero_is_the_kernels_error() {
        assert_eq!(item_length(88).expect("a length"), 88);
        let error = item_length(-libc::EINVAL).expect_err("an error");
        assert_eq!(error.raw_os_error(), Some(libc::EINVAL));
        let error = item_length(i32::MIN).expect_err("an error");
        assert_eq!(error.raw_os_error(), Some(libc::EINVAL));
    }

    #[test]
    fn an_answer_bigger_than_the_limit_is_refused() {
        assert_eq!(buffer(16).expect("a buffer"), vec![0; 16]);
        assert!(buffer(MAX_ANSWER + 1).is_err());
    }

    #[test]
    fn cuda_errors_name_the_call() {
        assert_eq!(check("cuInit", CUDA_SUCCESS), Ok(()));
        assert_eq!(
            check("cuInit", 100),
            Err("cuInit failed with CUDA error 100".to_owned())
        );
    }
}

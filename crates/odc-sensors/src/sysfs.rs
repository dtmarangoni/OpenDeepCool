//! Reading the kernel's one-number files, with the reason a read failed.

use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;

use crate::model::Unavailable;

/// Why reading `path` failed: this account may not read it, or anything else.
pub(crate) fn reason(path: &Path, error: &io::Error) -> Unavailable {
    match error.kind() {
        ErrorKind::PermissionDenied => Unavailable::PermissionDenied {
            path: path.to_owned(),
        },
        _ => Unavailable::ReadFailed {
            path: path.to_owned(),
        },
    }
}

/// A file holding one integer, as sysfs writes it.
pub(crate) fn read_integer<T: std::str::FromStr>(path: &Path) -> Result<T, Unavailable> {
    let text = fs::read_to_string(path).map_err(|e| reason(path, &e))?;
    text.trim().parse().map_err(|_| Unavailable::ReadFailed {
        path: path.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refused_read_is_permission_denied_and_any_other_error_a_failed_read() {
        let path = Path::new("/sys/fake/energy_uj");
        assert_eq!(
            reason(path, &io::Error::from(ErrorKind::PermissionDenied)),
            Unavailable::PermissionDenied { path: path.into() }
        );
        for kind in [
            ErrorKind::NotFound,
            ErrorKind::InvalidData,
            ErrorKind::Other,
        ] {
            assert_eq!(
                reason(path, &io::Error::from(kind)),
                Unavailable::ReadFailed { path: path.into() },
                "{kind:?}"
            );
        }
    }
}

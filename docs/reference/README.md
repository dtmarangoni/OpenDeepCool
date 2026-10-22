# Reference documents

Vendor manuals sit in `vendor/` **on the owner's machine only**. The folder is git-ignored because the documents are copyrighted by their vendors, and this repository is public. The facts this project needs from them are written up in our own words in [`../hardware/`](../hardware/).

| File (local only) | What it is | Source | SHA-256 |
|---|---|---|---|
| `vendor/deepcool-lp240-lp360-manual.pdf` | DeepCool LP240/LP360 (and WH) installation manual, 28 pages, dated 2024-08-15 in its metadata | Owner's copy (`LP240&360.pdf`) | `b3be3887bbfe6f21a91edcc7f2f17ceab363364bece9860c93efc1cfb995e965` |

Manuals for the owner's other hardware (e.g. the motherboard) are listed in `docs/local/`, which is git-ignored, because they reveal what the owner's PC is.

To recreate the folder on another machine, get the LP360 manual from the product box or from DeepCool, and check the hash before using it.

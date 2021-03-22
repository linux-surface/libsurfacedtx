# `libsurfacedtx`

![CI](https://github.com/linux-surface/libsurfacedtx/workflows/CI/badge.svg)

Library for Linux Surface DTX kernel driver user-space API.

The following crates are provided:
- `sdtx`: Main API wrapper.
- `sdtx-tokio`: [`tokio`][tokio] compatibility layer for asynchronous event handing.

Used by [`surface-control`][surface-control] and [`surface-dtx-daemon`][surface-dtx-daemon].

[tokio]: https://github.com/tokio-rs/tokio#tokio
[surface-control]: https://github.com/linux-surface/surface-control
[surface-dtx-daemon]: https://github.com/linux-surface/surface-dtx-daemon

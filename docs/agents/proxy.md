# proxy crate

Role: HTTP CONNECT handshake and proxy stream setup.

Key API:
- `connect_http_connect(proxy, target)`: performs CONNECT, returns TCP stream and leftover data.
- `connect_http_connect_with(proxy, target, options)`: allows SO_MARK and connect timeout control.

Tests:
- Success path with leftover data.
- Non-200 response rejection.

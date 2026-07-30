# Discovery

Goal of this tutorial:

- Verify that your host can **see** your GigE Vision camera.
- Run discovery from the `viva-camctl` CLI and from Rust.
- Understand the usual reasons it fails (NIC selection, firewall, subnets).

If discovery does not work, the other tutorials will not help much — fix this
first.

---

## Before you begin

Make sure that:

- The workspace builds:

  ```bash
  cargo build --workspace
  ```

- Your camera and host are physically connected — a direct cable from host NIC
  to camera, or a switch on a subnet dedicated to the cameras.
- The camera has a valid IPv4 address: from DHCP on your camera network, a
  static address matching the host NIC's subnet, or a link-local (APIPA)
  address. Link-local setups need a little extra care on the host — see
  [Link-local (APIPA) cameras](../networking.md#3-link-local-apipa-cameras).

For jumbo frames, MTU and throughput tuning, see the
[Networking Guide](../networking.md).

---

## Step 1 – Discover with `viva-camctl`

### 1.1. Basic discovery

```bash
cargo run -p viva-camctl -- list
```

On success you get one line per device with its IP, MAC, manufacturer and
model. If nothing appears:

- Check that the camera is powered and the link LED is lit.
- Check that your NIC is on the same subnet as the camera.
- Check that your host firewall allows UDP broadcast on that NIC.

### 1.2. Selecting an interface explicitly

On multi-NIC systems, tell `viva-camctl` which interface to use. The value is
the IPv4 address of your **host** NIC on the camera network, not the camera's:

```bash
cargo run -p viva-camctl -- list --iface 192.168.0.5
```

If you are not sure which NIC to use, `ip addr` (Linux), `ifconfig` (macOS) or
`ipconfig` (Windows) will tell you. The discovery output also names each
interface the library itself can see, which is not always the same set the OS
reports — an interface missing from that list is invisible to discovery no
matter what anything else says.

If discovery works with `--iface` but not without it, your machine has several
active interfaces and the automatic choice is not the one you expect.

### 1.3. Machine-readable output

`--json` is a top-level flag, so it goes **before** the subcommand:

```bash
cargo run -p viva-camctl -- --json list
```

---

## Step 2 – Discover from Rust

```bash
cargo run -p viva-genicam --example list_cameras
cargo run -p viva-genicam --example list_cameras -- --iface eth0
```

Note that the two `--iface` flags take different things: `viva-camctl` wants the
host NIC's **IPv4 address**, this example wants its **name**. That is worth
knowing before you conclude your interface is broken.

The part that matters is short:

```rust
{{#include ../../../crates/viva-genicam/examples/list_cameras.rs:discover}}
```

Three entry points exist, and the difference matters more than it looks:

| Function | Scans |
|---|---|
| `gige::discover(timeout)` | Every routable interface the library can enumerate |
| `gige::discover_on_interface(timeout, name)` | One named interface |
| `gige::discover_all(timeout)` | Every interface **including loopback** |

Use `discover_all` only when you are talking to the [fake
camera](./fake-camera.md) on `127.0.0.1`. Against real hardware it adds a
loopback scan that can only produce noise.

---

## Step 3 – Interpreting results

Record two things — you will reuse them in every later tutorial:

- The camera's IP address (e.g. `192.168.0.10`) → `--ip 192.168.0.10`
- The host NIC you used (e.g. `192.168.0.5`) → `--iface 192.168.0.5`

If several cameras answer, label them physically now rather than guessing later.

---

## Troubleshooting checklist

If neither `viva-camctl list` nor `list_cameras` finds anything:

1. **Physical link** — is the link LED lit on NIC, switch and camera? Try
   another cable or port.
2. **Subnets** — host NIC and camera must share a subnet. Two NICs on the same
   subnet confuse routing; avoid it.
3. **Firewall** — allow UDP broadcast on the camera NIC. On Windows the
   executable must be permitted for both "Private" and "Public" profiles. On
   Linux with firewalld, GVCP replies arrive from source port 3956 and need an
   explicit rule; see
   [Letting the reply back in](../networking.md#33-letting-the-reply-back-in-firewalld).
4. **Multiple NICs** — force the right one with `--iface <host-ip>`, or disable
   the others temporarily to confirm that NIC selection is the problem.
5. **Vendor tools** — if the vendor's viewer sees the camera and `viva-camctl`
   does not, compare which NIC and IP the vendor tool uses, and check whether it
   reconfigured the camera's address (DHCP, or a "force IP" button).

Still failing? Capture the details and send them:

```bash
cargo run -p viva-camctl -- report --out viva-report.txt
```

The report records your interfaces as the library sees them and everything
discovery did or did not hear — which is precisely what we need and cannot
guess. See [Reporting a camera we can't open](../reporting.md).

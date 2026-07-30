# Networking

This chapter is a practical **GigE Vision networking cookbook**.

It focuses on:

- Typical **topologies** (direct cable vs switch, single vs multi-camera).
- **NIC and IP configuration** on Windows, Linux, and macOS.
- **MTU / jumbo frames** and **packet delay** basics.
- Common **pitfalls and troubleshooting**.

It is not a replacement for vendor or A3 documentation, but gives you enough
background to make `viva-camctl` and the `viva-genicam` examples work reliably.  [oai_citation:0‡Wikipedia](https://en.wikipedia.org/wiki/GigE_Vision?utm_source=chatgpt.com)  

If you have not yet done so, first go through:

- [Discovery](./tutorials/discovery.md)
- [Streaming](./tutorials/streaming.md)

They show the CLI and Rust-side pieces that depend on a working network setup.

---

## 1. Typical topologies

### 1.1. Single camera, direct connection

The simplest and most robust setup:

```text
[Camera]  <── Ethernet cable ──>  [Host NIC]
```

Characteristics:
- One camera, one host, one NIC.
- No other traffic on that link.
- Easy to reason about MTU and packet delay.

Recommended when:
- You’re bringing up a new camera.
- You’re debugging issues and want to remove variables.

### 1.2. One or more cameras through a switch

Common in real systems:

```text
[Cam A] ──\
           \
[Cam B] ────[Switch]──[Host NIC]
           /
[Cam C] ─/
```

Characteristics:
- Multiple cameras share the link to the host.
- Switch must handle the aggregate throughput.
- Switch configuration (buffer sizes, jumbo frames, spanning tree) matters.  ￼

Recommended when:
- You need more than one camera.
- You need long cable runs or multi-drop layouts.

### 1.3. Host with multiple NICs

For high throughput or separation from office traffic:

```text
[Cam network]  <── NIC #1 ──>  [Host]  <── NIC #2 ──>  [Office / internet]
```

Characteristics:
- Camera traffic isolated from general network.
- Easier to tune MTU, QoS, and firewall rules.
- In discovery and streaming, you may need to specify --iface <host-ip>.

Recommended for:
- High data rates.
- Multi-camera setups.
- Systems that must not be disturbed by office network traffic.

⸻

## 2. IP addressing basics

GigE Vision uses standard IPv4 + UDP. Each device needs a valid IPv4 address; the
host and camera(s) must share a subnet.  ￼

### 2.1. Choose a camera subnet

Pick a private network, for example:
- 192.168.0.0/24 (addresses 192.168.0.1–192.168.0.254)
- 10.0.0.0/24

Decide on:
- One address for your host NIC (e.g. 192.168.0.5).
- One address per camera (e.g. 192.168.0.10, 192.168.0.11, …).

Make sure this subnet does not conflict with your office / internet network.

### 2.2. Windows
1.	Open Network & Internet Settings → Change adapter options.
2.	Right-click the NIC used for cameras → Properties.
3.	Select Internet Protocol Version 4 (TCP/IPv4) → Properties.
4.	Choose Use the following IP address:
	- IP address: e.g. 192.168.0.5
	- Subnet mask: 255.255.255.0
	- Gateway: leave empty (for isolated camera networks).
5.	Turn off any “energy saving” features for this NIC in the driver settings if
possible (they can introduce latency/jitter).

On first run, Windows firewall may pop up asking whether to allow the binary on
Private / Public networks. Allow it on the relevant profile so UDP broadcasts
work.

### 2.3. Linux

Use either NetworkManager or manual configuration.

Manual example:

```bash
# Assign IP and bring interface up (replace eth1 with your device)
sudo ip addr add 192.168.0.5/24 dev eth1
sudo ip link set eth1 up
```

To make this permanent, use your distro’s network configuration tools (e.g.
Netplan on Ubuntu, ifcfg files on RHEL, etc.).

### 2.4. macOS

Use System Settings → Network:
1.	Select the camera NIC (e.g. USB Ethernet).
2.	Set “Configure IPv4” to “Manually”.
3.	Enter:
	- IP address: 192.168.0.5
	- Subnet mask: 255.255.255.0
4.	Leave router/gateway empty for a dedicated camera network.

⸻

## 3. Link-local (APIPA) cameras

A GigE Vision camera with no static IP and no DHCP server on the segment falls
back to an IPv4 link-local address in `169.254.0.0/16` — what Windows calls
APIPA. This is the normal state of a camera plugged straight into a host with
nothing else configured, so it is worth knowing even if you plan to assign
static addresses later.

The library discovers such cameras on all platforms, but the **host** must hold
a link-local address of its own first, and on Linux the firewall usually has to
be told to let the reply back in.

> Most of this section comes from a bring-up performed by
> [@InsuJeong496](https://github.com/InsuJeong496) on a JAI FS-3200T-10GE-NNC
> and written up in
> [issue #57](https://github.com/VitalyVorobyev/viva-genicam/issues/57#issuecomment-5127958912).
> Their results: discovery succeeded, the MAC parsed correctly, the GenApi XML
> downloaded, 1 065 features loaded, and end-to-end streaming worked once the
> two firewall rules below were in place — with no vendor driver installed.

### 3.1. What is fixed and what is yours

The single most useful thing in that report was the distinction between the
values the protocol fixes and the values that belong to one particular machine.
Copying an address out of someone else's guide is the usual reason these
recipes fail.

Fixed — do not change these:

| Item | Value |
|---|---|
| IPv4 link-local network | `169.254.0.0/16` |
| Directed broadcast for that network | `169.254.255.255` |
| GVCP port on the camera | UDP `3956` |

Yours — substitute your own:

| Item | Where to get it |
|---|---|
| Host interface name | `ip -brief address` (Linux) |
| Host link-local address | Let the OS assign one, or pick an unused `169.254.x.y` |
| Camera address | Whatever `viva-camctl list` reports; it can change between sessions |
| firewalld zone | `firewall-cmd --get-active-zones` |
| GVSP destination port | `10040` unless you pass `--port` to `viva-camctl stream` |

### 3.2. Giving the host a link-local address (Linux)

Normally NetworkManager assigns one automatically when a link comes up with no
DHCP answer. If it has not, add one by hand:

```bash
# Replace enp9s0f3u3 with your camera NIC. Keep the /16 and the broadcast.
sudo ip address add 169.254.105.107/16 \
  broadcast 169.254.255.255 \
  dev enp9s0f3u3
```

Check what an interface currently has:

```bash
ip -brief address
```

Once the host has a link-local address, discovery sends its directed broadcast
to `169.254.255.255:3956`.

On Windows this step is normally automatic — an adapter with no DHCP lease
self-assigns a `169.254.x.y` address. On macOS the same is true, though see the
MTU note in [§4](#4-mtu-and-jumbo-frames): jumbo frames cannot be selected there.

### 3.3. Letting the reply back in (firewalld)

The most common symptom is that discovery finds nothing while the camera is
plainly on the link. The camera answers *from* UDP port 3956 to whatever
ephemeral port the client bound, so a default-deny inbound policy drops the ACK
and discovery simply times out.

Find the zone that owns the camera interface, then allow that source port:

```bash
firewall-cmd --get-active-zones

sudo firewall-cmd --zone=public \
  --add-rich-rule='rule family="ipv4" source address="169.254.0.0/16" source-port port="3956" protocol="udp" accept'
```

Replace `public` with your zone. Keep `169.254.0.0/16` and `3956`.

Streaming needs a second rule, because GVSP arrives on the port the host asked
the camera to send to — `10040` by default:

```bash
sudo firewall-cmd --zone=public --add-port=10040/udp
```

If you override the port, allow the one you actually use:

```bash
viva-camctl stream --ip <camera-ip> --iface <host-link-local-ip> --port <PORT>
sudo firewall-cmd --zone=<your-zone> --add-port=<PORT>/udp
```

**These are runtime rules.** They vanish on the next firewall reload or reboot
unless you repeat them with `--permanent`.

### 3.4. Checking it worked

```bash
viva-camctl --iface <host-link-local-ip> list
```

At `-v` the discovery log names the interface it sent from and the address it
heard back from:

```text
INFO sending GVCP discovery interface_name=enp9s0f3u3 local=169.254.105.107 dest=169.254.255.255:3956
INFO received GVCP response interface_name=enp9s0f3u3 src=169.254.253.222:3956
```

If the first line is missing, the host has no link-local address on that NIC
(§3.2). If the first line appears but the second never does, suspect the
firewall (§3.3).

⸻

## 4. MTU and jumbo frames

MTU (Maximum Transmission Unit) determines the largest Ethernet frame size.
Standard MTU is 1500 bytes; jumbo frames extend this (e.g. 9000 bytes). For
large images, jumbo frames can significantly reduce protocol overhead and CPU
load.  ￼

### 4.1. When to care

You probably need to look at MTU when:
- Frame sizes are large (multi-megapixel).
- Frame rates are high (tens or hundreds of FPS).
- You see lots of packet drops or resends at otherwise reasonable loads.

For simple bring-up and low/moderate data rates, standard MTU=1500 usually
works.

### 4.2. Enabling jumbo frames

All components in the path must agree:
- Camera
- Switch (if present)
- Host NIC

Typical steps:
- Camera: set `GevSCPSPacketSize` or similar feature to a value below the
path MTU (e.g. 8192 for MTU 9000). You can use viva-camctl set to adjust this.
- Switch: enable jumbo frames in the management UI (name and steps vary by
vendor).
- Host NIC:
    - Windows: NIC properties → Advanced → Jumbo Packet or similar.
    - Linux: sudo ip link set dev eth1 mtu 9000
    - macOS: some drivers expose MTU setting in the network settings; others do
not support jumbo frames.

After changing MTU, confirm with:

```bash
# Linux example
ip link show eth1
```

and check that TX/RX MTU matches your expectation.

⸻

## 5. Packet delay and flow control

Some cameras allow configuring inter-packet delay or packet interval:
- Without delay:
    - Camera sends packets as fast as possible.
    - High instantaneous bursts can overwhelm NICs / switches.
- With modest delay:
    - Traffic is smoother at the cost of a small increase in latency.

If you see high packet loss or many resends at high frame rates:
1.	Try slightly increasing the inter-packet delay.
2.	Observe:
    - Does the drop/resend rate decrease?
    - Is overall throughput still sufficient?

Some vendors also expose “frame rate limits” or “burst size” options. These can
also be used to ease pressure on the network at the cost of lower peak FPS.  ￼

⸻

## 6. Multi-camera considerations

When running multiple cameras:
- Total throughput is roughly the sum of each camera’s stream.
- The **bottleneck** can be:
    - The switch’s uplink to the host.
    - The host NIC’s capacity.
    - Host CPU / memory bandwidth.

Practical tips:
- Prefer a dedicated NIC for cameras.
- For 2–4 high-speed cameras, consider:
    - Multi-port NICs.
    - Separating cameras onto different NICs if possible.
- Stagger packet timing:
    - Slightly different inter-packet delays for each camera.
    - Slightly different frame rates, where acceptable.

Monitor:
- Per-camera stats (drops, resends, throughput).
- Host CPU usage.
- Switch port statistics if your hardware exposes them.

⸻

## 7. Using --iface and discovery quirks

On systems with more than one active NIC, automatic interface selection might
pick the wrong one.
- In viva-camctl, use --iface <host-ip> to force the correct NIC.
- In Rust examples, pass the desired local address when building the context
or stream (see the genicam and viva-gige crate chapters for details).

If discovery only works when you specify --iface, but not without it:
- You likely have:
    - Multiple NICs on overlapping subnets, or
    - A default route that prefers a different interface.
- This is not unusual; be explicit for production setups.

⸻

## 8. Troubleshooting checklist

Use this checklist when things don’t work as expected.

### 8.1. Discovery fails

See also the troubleshooting section in Discovery￼.
- Check link LEDs on camera, switch, and NIC.
- Confirm IP addressing:
    - Host and camera on same subnet.
    - No conflicting IPs.
- Check firewall:
    - Allow UDP broadcast / unicast on the camera NIC.
- Temporarily:
    - Disable other NICs to simplify routing.
    - Try a direct cable instead of a switch.

If the camera has a `169.254.x.y` address, go to
[§3 Link-local (APIPA) cameras](#3-link-local-apipa-cameras) instead — the
causes there are specific and the fixes are two commands.

If none of this helps, the camera itself is the evidence we need:
see [Reporting a camera we can't open](./reporting.md).

### 8.2. Streaming is unstable (drops / resends)
- Check MTU vs packet size; avoid exceeding path MTU.
- For high data rates:
    - Enable jumbo frames end-to-end (camera, switch, NIC).
- Reduce stress:
    - Lower frame rate or ROI.
    - Increase inter-packet delay slightly.
- Ensure dedicated NIC and switch where possible.
- Watch host CPU; if it’s near 100%, consider:
    - Better NIC / driver.
    - Moving processing off to another thread / core.

### 8.3. Vendor tool works, viva-genicam does not
Compare:
- Which NIC / IP the vendor tool uses.
   - The camera’s configured stream destination (IP/port).
   - The vendor tool might:
- Use a different MTU / packet size.
   - Adjust inter-packet delay automatically.
   - Try to replicate those parameters with viva-camctl and the NodeMap.

⸻

9. Recap

After this chapter you should:
	•	Understand basic GigE Vision network topologies and when to use each.
	•	Be able to configure a host NIC and camera addresses on Windows, Linux, and macOS.
	•	Know when and how to enable jumbo frames and adjust packet delay.
	•	Have a structured approach to debugging discovery and streaming issues.

For protocol-level details and tuning options exposed by this project:
	•	See viva-gige￼ for transport internals.
	•	See the Streaming tutorial￼ for concrete CLI and Rust examples.

---

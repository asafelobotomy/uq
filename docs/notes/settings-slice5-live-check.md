# Settings slice 5 (Sound, Bluetooth, Network): manual live check

> Not automated yet. Run on a real uQ session with PipeWire/WirePlumber, BlueZ and
> NetworkManager. Tick each item; write what you saw next to any that fails.

Before you start, note `user.lua`'s modification time (`stat -c %y ~/.config/hypr/uq/user.lua`).
It must be the same at the end (item 15).

## Sound

- [ ] **1. Devices.** Plug in headphones (or a USB/Bluetooth headset). The page lists them within
  a few seconds without a refresh. Pick them as the output: sound moves; volume and mute act on
  them only.
- [ ] **2. Microphone.** Pick a microphone, change its volume and mute it. `wpctl status` agrees.
- [ ] **3. Per app.** Play audio in two apps (a browser and `mpv`). Each shows under the app
  list; change one's volume and send it to another device. Close the app: its row goes.
- [ ] **4. Card mode.** Switch a card's mode (profile), for example HDMI output or a headset's
  hands-free mode, and back.
- [ ] **5. Test sound.** With `sound-theme-freedesktop` installed the test plays on the chosen
  device; without it the page says what to install.
- [ ] **6. No service.** `systemctl --user stop wireplumber pipewire pipewire-pulse`: the page
  says "Not available" with the package, and doesn't hang. Start them again: it recovers.

## Bluetooth

- [ ] **7. On and off.** Turn Bluetooth off and on from the page; quick settings agrees.
  Discoverable makes this computer visible from a phone.
- [ ] **8. Pair with a code.** Pair a phone or keyboard that shows a code: the page shows the
  same code and pairs only after **Confirm**. Cancel once: nothing is paired.
- [ ] **9. Connect, trust, forget.** Connect and disconnect a paired device; trust it and check it
  reconnects after turning it off and on; forget it. A device with a battery shows its level.
- [ ] **10. Adapter goes away.** `rfkill block bluetooth` (or unplug a USB dongle) while the page
  is open: no crash, the page says Bluetooth isn't available, and recovers after unblock.

## Network

- [ ] **11. Wi-Fi.** Turn Wi-Fi off and on; nearby networks list. Join a WPA2 network, then a
  hidden one by name. Forget one. The password never appears in `journalctl --user -b`.
- [ ] **12. Enterprise.** If you have one: a work or school (802.1X) network with a user name or
  certificate joins.
- [ ] **13. VPN.** Import a WireGuard `.conf`, edit a field, connect and disconnect. With
  `networkmanager-openvpn` installed, an OpenVPN connection shows and connects.
- [ ] **14. Other.** Start and stop a hotspot; set a static IPv4 address and DNS on a connection
  and put it back to automatic; mark a connection metered. "Everyone who uses this computer can
  join it too" unticked saves the password for you only (`nmcli -s c show <name>` as another
  user doesn't show it).
- [ ] **15. user.lua.** Its modification time is unchanged.

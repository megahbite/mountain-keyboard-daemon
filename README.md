# Mountain Keyboard Daemon
Adds extended support for Mountain Everest keyboard accessories on Linux.

## Building

Install a Rust development environment for your distro/OS

Build with `cargo build -r`

## Installation

The program needs write access to /dev/bus/usb/** devices. It can either be run as root, or preferably setup udev rules for login-user level access.

On Debian-based distros this can be done by creating a udev rule to give write access to the `plugdev` group:

Create a new file in /etc/udev/rules.d/:

```
echo "SUBSYSTEM==\"usb\", ATTRS{idVendor}==\"3282\", ATTRS{idProduct}==\"0001\", MODE=\"0660\", GROUP=\"plugdev\"" | sudo tee -a /etc/udev/rules.d/70-plugdev-mountain.rules >/dev/null
sudo udevadm trigger --subsystem-match=usb
```

Adjust the group as necessary on non-Debian based distros.

Then run `./mountain-keyboard-daemon` from wherever you downloaded or built to.

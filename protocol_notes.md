# Overview

* Vendor ID: 0x3282 (360 Service Agency GmbH)
* Device ID: 0x0001 (Everest Keyboards?)

* USB API I/O is on interface 0x03 which is an HID interface with two endpoints, in (0x84) and out (0x05)
* 64 byte packets
* Endpoint 0x81 on main interface (0x00) is the boot endpoint
* Endpoint 0x82 (interface 0x01) is media controls.
* Endpoint 0x83 (interface 0x02) is the keyboard input.

## API

* First byte is nearly always 0x11 (HID "vendor" related? - 0x01 seems to be used for normal keyboard input)
* First byte is sometimes 0xFF
* Second byte seems to be some sort of op. code.
    * 0x14 gets sent at one second intervals to 0x05, receiving some data back from 0x84. Initially it only seems to send back packets starting with 0x01 and are otherwise empty. It feels like some kind of device/ready detection + possibly caps (attached accessories and config for instance). The value changes when the top accessory is docked.
        * Looking further into this, the keyboard will sometimes start returning 0x01 packets again. In which case the host sends an 0x11 with no op. code or data and the keyboard sends back a similar reply. The host follows up with a 11:02 packet with some data and the keyboard replies with FF:AA:FF. Like so (only first 11 bytes have any data):
            ```
                     00 01 02 03 04 05 06 07 08 09 0A
            1. host: 11 14 00 00 00 00 00 00 00 00 00
            2. keyb: 01 00 00 00 00 00 00 00 00 00 00
            3. host: 11 00 00 00 00 00 00 00 00 00 00
            4. keyb: 11 00 00 00 57 00 00 00 06 00 01
            5. host: 11 02 00 01 01 2c 02 00 00 00 00
            6. keyb: ff aa ff 00 00 00 00 00 00 00 00
            ```
            The data in 5 from the host does seem to change for an unknown reason. 4 from the keyboard is consistent.

            This has not occurred in all the packet dumps I have captured so it's triggered by something.

        * I had a look at a capture of when a macro is triggered and interestingly, instead of the keyboard replying with the FF:AA:FF, it instead replied with the macro string with a 0x02 op. code. It was spread over two replies which might explain the quirk with always needing two interrupts in flight. I need to check what the limit of the macro length is.

    * An initial handshake seems to occur like so:
        
        ```
                 00 01 02 03 04 05 06 07 08 09 0A 0B
        1. host: 11 80 00 00 01 00 00 00 00 00 00 00
        2. keyb: 11 80 00 00 01 00 00 00 00 00 00 00
        3. host: 11 84 00 00 00 00 00 00 00 00 00 00
        4. keyb: 11 84 00 00 00 00 00 00 00 00 00 01
        5. host: 11 84 00 01 00 00 XX XX XX XX XX 01
        6. keyb: 11 84 00 01 00 00 XX XX XX XX XX 01
        ```
        ~~0x84 seems to give a 32bit time stamp update to the device.~~
        
        ~~There are 5 bytes of changing data so now I'm not sure. It possibly is a full 64bit, covering from the 2nd byte to the 9th, or less likely from the 3rd to the 10th.~~
        
        ~~Unix Epoch doesn't make a lot of sense here, so probably from 1900 (Windows Epoch).~~
        
        It's even more mundane than I expected. It's not an epoch, simply 5 bytes for:
        * **06** Month: 1-12 (it wraps from testing)
        * **07** Day: 1-31 (yes you can have February 31st)
        * **08** Hour: 0-23 (24hr time)
        * **09** Minute: 0-59
        * **0A** Seconds: 0-59 (not displayed but it will updated the time on its own based on this)

        Not data efficient and a pain to convert to, but would be extremely simple to display and means an embedded system wouldn't need to care about datetime complexities.

    * 0x81 and 0x83 seem related to the display dial. I think hardware stats and volume display.
        * 0x81 is the hardware stats. First byte after is 0-4 which are the different hardware modes (CPU %, GPU %, HDD % (??? is this how saturated throughput is?), Internet throughput in MB/s (remember megabytes, not bits), and RAM consumption %), then a 16bit value (to allow for gigabit internet speeds presumably). What I haven't worked out is how the keyboard is notifying it is switching between them. It does not initiate any state change through op. codes. It's probably in the keepalive data.
        * 0x83 is the volume level. I think this can be safely sent any time the system's volume is changed. Will have to test.
    * Have yet to capture whatever an 0x82 op. code or any others may cover but haven't explored all the functions of the media dock.
* Weird gotcha in that you have to send an IN interrupt before you push any data to OUT in order to get a response from the device. Sending it post packet doesn't seem to work. Also have to have two in-flight or it just never replies.
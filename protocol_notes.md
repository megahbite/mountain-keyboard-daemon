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
* Second byte seems to be some sort of op code.
    * 0x14 gets sent at one second intervals to 0x05, receiving some data back from 0x84. Initially it only seems to send back packets starting with 0x01 and are otherwise empty. It feels like some kind of device/ready detection + possibly caps (attached accessories and config for instance). The value changes when the top accessory is docked.
    * An initial handshake seems to occur like so:
        
        |0 |1 |2 |3 |4 |5 |6 |7 |8 |9 |A |B |Direction from Host|
        |--|--|--|--|--|--|--|--|--|--|--|--|-------------------|
        |80|00|00|01|00|00|00|00|00|00|00|00| -> OUT            |
        |80|00|00|01|00|00|00|00|00|00|00|00| <- IN             |
        |84|00|00|00|00|00|00|00|00|00|00|00| -> OUT            |
        |84|00|00|00|00|00|00|00|00|00|01|00| -> IN             |
        |84|00|01|00|00|00|XX|XX|XX|XX|01|00| -> OUT            |
        |84|00|01|00|00|00|XX|XX|XX|XX|01|00| -> IN             |

        0x84 seems to give a 32bit time stamp update to the device.
* Weird gotcha in that you have to send an IN interrupt before you push any data to OUT in order to get a response from the device. Sending it post packet doesn't seem to work. Also have to have two in-flight or it just never replies.
* 0x81 and 0x83 seem related to the display dial. I think hardware stats and volume display.
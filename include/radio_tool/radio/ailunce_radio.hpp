/**
 * This file is part of radio_tool.
 * Copyright (c) 2022 Niccol� Izzo IU2KIN
 * Copyright (c) 2022 v0l <radio_tool@v0l.io>
 * 
 * radio_tool is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * radio_tool is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 * 
 * You should have received a copy of the GNU General Public License
 * along with radio_tool. If not, see <https://www.gnu.org/licenses/>.
 */
#pragma once

#include <radio_tool/radio/radio.hpp>
#include <radio_tool/device/ymodem_device.hpp>

#include <libusb-1.0/libusb.h>

namespace radio_tool::radio
{
    class AilunceRadio : public RadioOperations
    {
    public:
        // Prolific Technology, Inc. - USB-Serial Controller
        static const auto VID = 0x067b;
        static const auto PID = 0x2303;

        AilunceRadio(const std::string &prt, const std::string &fname)
            : device(prt, fname) {}

        auto WriteFirmware(const std::string &file) const -> void override;
        auto ToString() const -> const std::string override;

        auto GetDevice() const -> const device::RadioDevice* override
        {
            return &device;
        }

        static auto SupportsDevice(const std::string &) -> bool
        {
            // not possible to detect from serial port?
            // ideally we could map serial ports to USB devices to validate VID:PID
            //
            // possible windows solution: https://aticleworld.com/get-com-port-of-usb-serial-device/
            // possible linux solution: https://unix.stackexchange.com/a/81767
            return true;
        }

        static auto Create(const std::string &port) -> const AilunceRadio*
        {
            return new AilunceRadio(port, "firmware.bin");
        }

    private:
        device::YModemDevice device;
    };
} // namespace radio_tool::radio

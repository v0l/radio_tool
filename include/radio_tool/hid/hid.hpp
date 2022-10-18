/**
 * This file is part of radio_tool.
 * Copyright (c) 2021 v0l <radio_tool@v0l.io>
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

#include <vector>
#include <libusb-1.0/libusb.h>

namespace radio_tool::hid
{
    class HID
    {
    public:
        HID(libusb_device_handle *device)
            : timeout(5000), device(device) {
            }

        auto InterruptRead(const uint8_t &ep, const uint16_t &len) const -> std::vector<uint8_t>;
        auto InterruptWrite(const uint8_t &ep, const std::vector<uint8_t>&) const -> void;

        auto BulkRead(const uint8_t &ep, const uint16_t &len) const -> std::vector<uint8_t>;
        auto BulkWrite(const uint8_t &ep, const std::vector<uint8_t>&) const -> void;
    protected:
        const uint16_t timeout;
        libusb_device_handle *device;


        auto HandleEvents() const -> void;
    };
};
/**
 * This file is part of radio_tool.
 * Copyright (c) 2020 v0l <radio_tool@v0l.io>
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
#include <radio_tool/radio/radio_factory.hpp>
#include <radio_tool/radio/usb_radio_factory.hpp>
#include <radio_tool/radio/serial_radio_factory.hpp>

#include <functional>

using namespace radio_tool::radio;

auto RadioFactory::GetRadioSupport(const uint16_t &idx) const -> const RadioOperations*
{
    return nullptr;
}

auto RadioFactory::ListDevices(const uint16_t &) const -> const std::vector<RadioInfo*>
{
    uint16_t idx_offset = 0;
    auto ret = std::vector<RadioInfo*>();

    auto usb = USBRadioFactory();
    auto usbDevices = usb.ListDevices(idx_offset);
    ret.insert(ret.end(), usbDevices.begin(), usbDevices.end());
    idx_offset += (uint16_t)usbDevices.size();

    auto serial = SerialRadioFactory();
    auto serialDevices = serial.ListDevices(idx_offset);
    ret.insert(ret.end(), serialDevices.begin(), serialDevices.end());
    
    return ret;
}
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
#include <radio_tool/radio/ailunce_radio.hpp>
#include <radio_tool/fw/ailunce_fw.hpp>

#include <stdio.h>
#include <unistd.h>

using namespace radio_tool::radio;

auto AilunceRadio::ToString() const -> const std::string
{
    return "== Ailunce USB Serial Cable ==";
}

auto AilunceRadio::WriteFirmware(const std::string &file) const -> void
{
    auto fw = fw::AilunceFW();
    fw.Read(file);

    //XOR raw binary data before sending
    fw.Encrypt();

    auto fd = device.GetFD();
    write(fd, "1", 1); // send 1 to start firmware upgrade
    usleep(1000000);   // sleep enough to transmit the 1

    auto r = fw.GetDataSegments()[0];
    device.SetInterfaceAttribs(57600, 0);
    device.Write(r.data);
}
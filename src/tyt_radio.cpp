/**
 * This file is part of radio_tool.
 * Copyright (c) 2020 Kieran Harkin <kieran+git@harkin.me>
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
#include <radio_tool/radio/tyt_radio.hpp>

#include <math.h>

using namespace radio_tool::radio;

auto TYTRadio::Open(const uint16_t &idx) -> bool
{
    dev_index = idx;
    if (dfu.Init())
    {
        dfu.Open(idx);
        return true;
    }
    return false;
}

auto TYTRadio::WriteFirmware(const std::string &file) const -> void
{
    //fw.Read(file);

    for (auto &r : fw.GetMemoryRanges())
    {
        auto nBlocks = ceil(r.second / BlockSize);
        for (auto wValue = 2; wValue < nBlocks; wValue++)
        {
        }
    }
}
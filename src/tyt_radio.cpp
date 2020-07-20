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
#include <radio_tool/fw/tyt_fw.hpp>

#include <math.h>
#include <iomanip>
#include <iostream>
#include <vector>

using namespace radio_tool::radio;

auto TYTRadio::WriteFirmware(const std::string &file) const -> void
{
    auto fw = fw::TYTFW();
    fw.Read(file);

    constexpr auto WipeBlockSize = 0x10000;

    auto GetSector = [](const uint32_t& addr) {
        //<start>, <sector-size>, <n-sectors>
        const std::vector<std::tuple<uint32_t, uint16_t, uint8_t>> MemMapSTM32F405 = {
            /* Aliased flash memory */
            { 0x00000000, 0x000fffff, 1},

            /* FLASH */
            { 0x08000000, 16000, 4 },
            { 0x08010000, 64000, 1 },
            { 0x08020000, 128000, 6 }
        };
        auto sec_offset = 0;
        for(const auto& rx : MemMapSTM32F405) {
            auto range_begin = std::get<0>(rx);
            auto sector_size = std::get<1>(rx);
            auto sectors = std::get<2>(rx);
            auto range_max = range_begin + (sector_size * sectors);
            if(addr > range_begin && addr < range_max) {
                auto in_sector = ((addr - range_begin) / sector_size) + sec_offset;
                return std::make_pair(in_sector, rx);
            }
            else 
            {
                sec_offset += sectors;
            }
        }
        throw std::runtime_error("Invalid address");
    };

    for (auto &r : fw.GetMemoryRanges())
    {
        auto end = r.first + r.second;
        for (auto a = r.first; a < end + (end % WipeBlockSize); a += WipeBlockSize)
        {
            auto sec_info = GetSector(a);
            std::cerr << "Wiping: 0x" << std::setw(8) << std::setfill('0') << std::hex << a << std::endl;
            dfu.Erase(a);
        }
    }

    for (auto &r : fw.GetMemoryRanges())
    {
        auto nBlocks = ceil(r.second / BlockSize);
        for (auto wValue = 2; wValue < nBlocks; wValue++)
        {
            
        }
    }
}
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
#pragma once

#include <vector>
#include <optional>
#include <iomanip>
#include <functional>
#include <algorithm>

#include <stdint.h>

namespace radio_tool::flash
{
    class FlashSector
    {
    public:
        FlashSector(const uint16_t &idx, const uint32_t &sector_start, const uint32_t &sector_size)
            : index(idx), start(sector_start), size(sector_size) {}

        /**
         * Flash sector index (0-N usually)
         */
        const uint16_t index;

        /**
         * Start address of this flash sector
         */
        const uint32_t start;

        /**
         * Size of this flash sector
         */
        const uint32_t size;

        /**
         * End address of this flash sector
         */
        constexpr auto End() const -> const uint32_t
        {
            return start + size;
        }

        /**
         * If this addr is inside this sector
         */
        constexpr auto InSector(const uint32_t &addr) const -> bool
        {
            return addr >= start && addr < End();
        }

        /**
         * Get a string describing this sector
         */
        auto ToString() const -> const std::string 
        {
            std::stringstream out;
            out << "FlashSector[Index=" << index
                << ", Start=0x" << std::setw(8) << std::setfill('0') << std::hex << start
                << ", End=0x" << std::setw(8) << std::setfill('0') << std::hex << End()
                << "]";

            return out.str();
        }
    };


    typedef std::vector<FlashSector> FlashMap;

    class FlashUtil
    {
    public:
        /**
         * Get the sector of an address in a flash map
         */
        static auto GetSector(const FlashMap &map, const uint32_t &addr) -> std::optional<const FlashSector>
        {
            for (const auto &sec : map)
            {
                if (sec.InSector(addr))
                {
                    return sec;
                }
            }
            return {};
        }

        /**
         * Create a simple memory layout with all sectors having the same size
         */
        static auto MakeSimpleLayout(const uint32_t& start_addr, const uint32_t& sector_size, const uint16_t& sectors) -> FlashMap
        {
            auto ret = FlashMap();
            for(auto x = 0; x < sectors; x++) 
            {
                ret.push_back(FlashSector(x, start_addr + (sector_size * x), sector_size));
            }
            return ret;
        }

        /**
         * Executes a function, sector aligned over a range of bytes for a give map
         */
        static auto AlignedContiguousMemoryOp(const FlashMap& map, const uint32_t& start, const uint32_t& end,
            const std::function<void(const uint32_t&, const uint32_t&, const FlashSector&)>& fnOp) -> void 
        {
            for (auto addr = start; addr < end;)
            {
                if(const auto& sec_info = flash::FlashUtil::GetSector(map, addr)) 
                {
                    auto n_bytes = std::min(end, sec_info->End()) - addr;

                    fnOp(addr, n_bytes, sec_info.value());

                    addr += n_bytes;
                } 
                else 
                {
                    break; //unmapped region
                }
            }
        }
    };

    /**
     * STM32F40X & STM32F41X Memory organization
     */
    const FlashMap STM32F40X = {
        {0, 0x08000000, 0x4000}, /* 16k */
        {1, 0x08004000, 0x4000},
        {2, 0x08008000, 0x4000},
        {3, 0x0800c000, 0x4000},
        {4, 0x08010000, 0x10000}, /* 64k */
        {5, 0x08020000, 0x20000}, /* 128k */
        {6, 0x08040000, 0x20000},
        {7, 0x08060000, 0x20000},
        {8, 0x08080000, 0x20000},
        {9, 0x080a0000, 0x20000},
        {10, 0x080c0000, 0x20000},
        {11, 0x080e0000, 0x20000}
    };

    /**
     * Winbond W25Q128JV SPI Flash (16MB)
     * 256 Blocks
     * 4k Sector size
     * 16 Sectors
     * 
     * (16 * 4k) * 256
     * Used in: DM1701, (Others?)
     */
    const FlashMap W25Q128JV = FlashUtil::MakeSimpleLayout(0x00, 0x10000, 0x100);

    /**
     * Micron M25P16 SPI Flash (2MB)
     * 65k Sector size (512Kbit)
     * 32 Sectors
     * 
     * 32 * 64k
     */
    const FlashMap M25P16 = FlashUtil::MakeSimpleLayout(0x00, 0x10000, 0x20);

} // namespace radio_tool::flash

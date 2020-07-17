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
#pragma once

#include <string>
#include <vector>

namespace radio_tool::fw
{
    class FirmwareSupport
    {
    public:
        virtual ~FirmwareSupport() = default;

        virtual auto Read(const std::string& fw) -> void = 0;
        virtual auto Write(const std::string& fw) -> void = 0;
        virtual auto ToString() const -> std::string = 0;

        auto GetData() const -> const std::vector<uint8_t>& {
            return data;
        }
        auto GetMemoryRanges() const -> const std::vector<std::pair<uint32_t, uint32_t>>& {
            return memory_ranges;
        }
    protected:
        /**
         * The firmware binary
         */
        std::vector<uint8_t> data;

        /**
         * Memory ranges to write the firmware file to
         * <Address, Length>
         */
        std::vector<std::pair<uint32_t, uint32_t>> memory_ranges;
    };
} // namespace radio_tool::fw
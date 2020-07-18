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

        /**
         * Read the firmware file from disk
         */
        virtual auto Read(const std::string& fw) -> void = 0;

        /**
         * Write the firmware file to disk
         */
        virtual auto Write(const std::string& fw) -> void = 0;

        /**
         * Returns general info about the firmware file
         */
        virtual auto ToString() const -> std::string = 0;

        /**
         * Returns the radio model this firmware file is for
         */
        virtual auto GetRadioModel() const -> const std::string = 0;

        /**
         * Gets the firmware binary
         */
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
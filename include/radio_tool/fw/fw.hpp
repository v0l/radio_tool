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
    class FirmwareSegment
    {
    public:
        FirmwareSegment(const uint16_t &idx, const uint32_t &addr, const uint32_t &size, const std::vector<uint8_t>::const_iterator &begin, const std::vector<uint8_t>::const_iterator &end)
            : index(idx), address(addr), size(size), data(begin, end)
        {
        }

        /**
         * Index of the segment
         */
        const uint16_t index;

        /**
         * The address the segment should be written to on the device
         */
        const uint32_t address;

        /**
         * The size of the data segment
         */
        const uint32_t size;

        /**
         * A copy of the data from the firwmare file
         */
        const std::vector<uint8_t> data;
    };

    class FirmwareSupport
    {
    public:
        virtual ~FirmwareSupport() = default;

        /**
         * Read the firmware file from disk
         */
        virtual auto Read(const std::string &fw) -> void = 0;

        /**
         * Write the firmware file to disk
         */
        virtual auto Write(const std::string &fw) -> void = 0;

        /**
         * Returns general info about the firmware file
         */
        virtual auto ToString() const -> std::string = 0;

        /**
         * Returns the radio model this firmware file is for
         */
        virtual auto GetRadioModel() const -> const std::string = 0;

        /**
         * Set the radio model this firmware file is for
         */
        virtual auto SetRadioModel(const std::string&) -> void = 0;

        /**
         * Decrypt the firmware data
         */
        virtual auto Decrypt() -> void = 0;

        /**
         * Encrypt the firmware data
         */
        virtual auto Encrypt() -> void = 0;

        /**
         * Gets the firmware binary
         */
        auto GetData() const -> const std::vector<uint8_t> &
        {
            return data;
        }

        /**
         * Get segments to write in the firmware
         */
        virtual auto GetDataSegments() const -> const std::vector<FirmwareSegment>
        {
            std::vector<FirmwareSegment> ret;

            auto r_idx = 0u;
            auto r_offset = 0u;
            for(const auto& r : memory_ranges) 
            {
                ret.push_back(FirmwareSegment(
                    r_idx++, 
                    r.first, 
                    r.second, 
                    data.begin() + r_offset,
                    data.begin() + r_offset + r.second
                ));
                r_offset += r.second;
            }

            return ret;
        }

        /**
         * Adds a data segment to this firmware
         * @note Normally used when wrapping new firmware
         * @remarks Data will be padded if its too short
         */
        virtual auto AppendSegment(const uint32_t &addr, const std::vector<uint8_t> &new_data) -> void
        {
            constexpr auto allign = 0x200u;
            auto extra = new_data.size() % allign;
            auto new_size = new_data.size() + (extra > 0 ? allign - extra : 0);
            data.reserve(data.size() + new_size);
            std::copy(new_data.begin(), new_data.end(), std::back_inserter(data));
            if(extra > 0)
            {
                std::fill_n(std::back_inserter(data), allign - extra, 0xff);
            }
            memory_ranges.push_back({addr, new_size});
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
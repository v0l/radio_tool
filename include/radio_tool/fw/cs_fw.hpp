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
 * 
 * Some or all of this file is copied from CSFWTOOL
 * https://github.com/KG5RKI/CSFWTOOL
 */
#pragma once

#include <radio_tool/fw/fw.hpp>

#include <memory>

namespace radio_tool::fw
{
    typedef struct
    {
        uint32_t baseaddr_offset; // 0x00 - 0x08000000 + this (0x20000 or 0x10000 usually), 0 if spi flash data
        uint32_t unkaddr1;        // 0x04 - 0
        uint32_t unkaddr2;        // 0x08 - 0
        uint32_t rsrc_size;       // 0x0C - 0 if image bin, size if spi flash data (ex. 0x00069010 for RCDB)
        uint32_t unksize1;        // 0x10 - 0
        uint32_t imagesize;       // 0x14 - total image data size is this + imageHeaderSize
        uint8_t padding1[0x24];   // 0x18
        uint32_t rsrcHeaderSize;  // 0x3C - 0x80 if spi data, 0 if image data
        uint32_t unkHeaderSize;   // 0x40 - 0
        uint32_t imageHeaderSize; // 0x44 - 0x80
        uint8_t padding2[0x24];   // 0x48
        uint32_t version;         // 0x6C - 0x00000001 usually
        uint8_t resv[0x10];       // 0x70
    } CS800D_header;              // total size 0x80
    static_assert(sizeof(CS800D_header) == 0x80);

    class CSFW : public FirmwareSupport
    {
    public:
        auto Read(const std::string &fw) -> void override;
        auto Write(const std::string &fw) -> void override;
        auto ToString() const -> std::string override;
        auto GetRadioModel() const -> const std::string override;
        auto SetRadioModel(const std::string&) -> void override;
        auto Decrypt() -> void override;
        auto Encrypt() -> void override;

        /**
         * Tests a file if its a valid firmware file
         */
        static auto SupportsFirmwareFile(const std::string &file) -> bool;

        static auto SupportsRadioModel(const std::string &model) -> bool;

        /**
         * Create an instance of this class for the firmware factory
         */
        static auto Create() -> std::unique_ptr<FirmwareSupport>
        {
            return std::make_unique<CSFW>();
        }
    private:
        CS800D_header header;
        uint16_t checksum;

        auto MakeChecksum() const -> const uint16_t;
        auto MakeFiledata() const -> std::vector<uint8_t>;
    };
} // namespace radio_tool::fw
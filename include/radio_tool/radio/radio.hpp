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

#include <radio_tool/dfu/dfu.hpp>

#include <string>
#include <iomanip>

namespace radio_tool::radio
{
    class RadioInfo
    {
    public:
        const std::wstring manufacturer, product;
        const uint16_t vid, pid, index;

        RadioInfo(std::wstring mfg, std::wstring prd, uint16_t vid, uint16_t pid, uint16_t idx)
            : manufacturer(mfg), product(prd), vid(vid), pid(pid), index(idx) {}

        auto ToString() const -> const std::wstring
        {
            std::wstringstream os;
            os << L"["
               << std::setfill(L'0') << std::setw(4) << std::hex << vid
               << L":"
               << std::setfill(L'0') << std::setw(4) << std::hex << pid
               << L"]: idx=" << std::setfill(L'0') << std::setw(3) << std::to_wstring(index) << L", "
               << manufacturer << L" " << product;
            return os.str();
        }
    };

    class RadioSupport
    {
    public:
        virtual ~RadioSupport() = default;

        /**
         * Write a firmware file to the device (Firmware Upgrade)
         */
        virtual auto WriteFirmware(const std::string &file) const -> void = 0;
        
        //virtual auto WriteCodeplug();
        //virtual auto ReadCodeplug();

        /**
         * Return the device communication handler
         */
        virtual auto GetDFU() const -> const dfu::DFU& = 0;
    };
} // namespace radio_tool::radio
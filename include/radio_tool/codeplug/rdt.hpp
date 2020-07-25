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

#include <radio_tool/codeplug/codeplug.hpp>
#include <radio_tool/codeplug/rdt_header.hpp>
#include <radio_tool/codeplug/rdt_general.hpp>

#include <string>
#include <fstream>
#include <memory>

namespace radio_tool::codeplug 
{    
    class RDT : public CodeplugSupport {
    public:
        static auto SupportsCodeplug(const std::string &file) -> bool
        {
            std::ifstream file_test(file, std::ios_base::in | std::ios_base::binary);
            if(file_test.is_open()) 
            {
                auto hdr = RDTHeader();
                hdr.Read(file_test);
                file_test.close();

                return hdr.Validate();
            }
            return false;
        }

        static auto Create() -> std::unique_ptr<RDT>
        {
            auto nInst = new RDT();
            return std::unique_ptr<RDT>(nInst);
        }

        auto Read(const std::string&) -> void override;
        auto Write(const std::string&) const -> void override;
        auto GetData() const -> const std::vector<uint8_t> override;
        auto ToString() const -> const std::string override;
    private:
        RDTHeader header;
        time_t timestamp;
        RDTGeneral general;
    };
}
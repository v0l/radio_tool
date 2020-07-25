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
#include <radio_tool/codeplug/rdt.hpp>
#include <radio_tool/util.hpp>

#include <sstream>

using namespace radio_tool::codeplug;

auto RDT::Read(const std::string &file) -> void
{
    std::ifstream file_read(file, std::ios_base::in | std::ios_base::binary);
    if (file_read.is_open())
    {
        header.Read(file_read);

        //skip to timestamp
        file_read.seekg(header.GetTimestampOffset(), std::ios_base::cur);

        uint8_t ts_data[7];
        file_read.read((char*)ts_data, 7);

        timestamp = ParseBCDTimestamp(ts_data);
    }
    else
    {
        throw std::runtime_error("Cant open file");
    }
    
}

auto RDT::Write(const std::string &) const -> void
{
}

auto RDT::GetData() const -> const std::vector<uint8_t>
{
}

auto RDT::ToString() const -> const std::string
{
    std::stringstream out;

    out
        << " == RDT Codeplug ==" << std::endl
        << "Radio:   " << header.radio << std::endl
        << "Created: " << ctime(&timestamp) //<< std::endl
        << "Target:  " << header.target_name << std::endl;
    return out.str();
}
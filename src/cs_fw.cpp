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
#include <radio_tool/fw/cs_fw.hpp>
#include <radio_tool/fw/cipher/cs800.hpp>
#include <radio_tool/fw/cipher/dr5xx0.hpp>
#include <radio_tool/util.hpp>

#include <fstream>
#include <sstream>
#include <iomanip>
#include <iterator>

using namespace radio_tool::fw;

auto CSFW::Read(const std::string &fw) -> void
{
    std::ifstream in_file(fw, std::ios_base::binary);
    if(in_file.is_open())
    {
        in_file.seekg(0, std::fstream::end);
        auto len = in_file.tellg();
        in_file.seekg(0, std::fstream::beg);

        in_file.read((char*)&header, sizeof(CS800D_header));

        //test file size is correct
        if(header.imagesize == 0)
        {
            throw std::runtime_error("Invalid firmware file");
        }
        if(header.imagesize + header.imageHeaderSize + sizeof(uint16_t) != len)
        {
            throw std::runtime_error("Invalid firmware header");
        }

        data.resize(header.imagesize);
        in_file.read((char*)data.data(), header.imagesize);
        in_file.read((char*)&checksum, sizeof(uint16_t));
        in_file.close();

        memory_ranges.push_back({header.baseaddr_offset, header.imagesize});

        //test checksum
        auto cs_check = MakeChecksum();
        if(cs_check != checksum)
        {
            //checksum not working right now
            //throw std::runtime_error("Invalid checksum");
        }
    }
    else 
    {
        throw std::runtime_error("Cant open file");
    }
}

auto CSFW::Write(const std::string &fw) -> void
{
    std::ofstream of(fw, std::ios_base::binary);
    if(of.is_open())
    {
        auto data = MakeFiledata();
        of.write((char*)data.data(), data.size());

        auto iter = data.begin();
        auto cs = CSChecksum(iter, data.size());
        of.write((char*)&cs, sizeof(cs));

        of.close();
    }
}

auto CSFW::ToString() const -> std::string
{
    std::stringstream out;

    out << "== Connect Systems Firmware ==" << std::endl
        << "Image Size: " << std::fixed << std::setprecision(2) << (header.imagesize / 1024.0) << " KiB" << std::endl
        << "Version:    " << header.version << std::endl
        << "Checksum:   0x" << std::setw(4) << std::setfill('0') << std::hex << checksum << std::endl
        << "Data Segments: " << std::endl;

    auto n = 0u;
    for (const auto &m : memory_ranges)
    {
        out << "  " << n++ << ": Start=0x" << std::setfill('0') << std::setw(8) << std::hex << m.first
            << ", Length=0x" << std::setfill('0') << std::setw(8) << std::hex << m.second
            << std::endl;
    }

    return out.str();
}

auto CSFW::GetRadioModel() const -> const std::string
{
    //TODO: find a way to detect firmware radio model
    return "CS800"; 
}


auto CSFW::SetRadioModel(const std::string&) -> void
{
    //dont do anything (yet)
}

auto CSFW::Decrypt() -> void
{
    //dont know how to detect dr5xx0 so just use cs800 cipher always
    ApplyXOR(data, cipher::cs800, cipher::cs800_length);
}

auto CSFW::Encrypt() -> void
{
    //dont know how to detect dr5xx0 so just use cs800 cipher always
    ApplyXOR(data, cipher::cs800, cipher::cs800_length);
}

auto CSFW::SupportsFirmwareFile(const std::string &file) -> bool
{
    std::ifstream in_file(file, std::ios_base::binary);
    if(in_file.is_open())
    {
        CS800D_header header = {};
        in_file.read((char*)&header, sizeof(CS800D_header));
        in_file.seekg(0, std::ios_base::end);
        auto len = in_file.tellg();
        in_file.close();

        //test is not resource file
        if(header.imagesize == 0)
        {
            return false;
        }

        //test image size matches
        if(header.imagesize + header.imageHeaderSize + sizeof(uint16_t) != len)
        {
            return false;
        }

        return true;
    }
    return false;
}

auto CSFW::SupportsRadioModel(const std::string &model) -> bool
{
    //TODO: Make this better
    if(model == "CS800")
    {
        return true;
    }
    return false;
}

auto CSFW::MakeChecksum() const -> const uint16_t
{
    auto to_check = MakeFiledata();
    auto iter = to_check.begin();
    auto cs = CSChecksum(iter, to_check.size());
    return cs;
}

auto CSFW::MakeFiledata() const -> std::vector<uint8_t>
{
    std::vector<uint8_t> to_check;
    to_check.reserve(header.imageHeaderSize + header.imagesize);
    for(auto ih = 0; ih < sizeof(CS800D_header); ih++)
    {
        to_check.push_back(((uint8_t*)&header)[ih]);
    }
    for(const auto& d : data)
    {
        to_check.push_back(d);
    }
    return to_check;
}
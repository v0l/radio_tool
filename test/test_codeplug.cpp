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
#include <radio_tool/codeplug/codeplug_factory.hpp>
#include <radio_tool/codeplug/rdt_header.hpp>
#include <radio_tool/codeplug/rdt_general.hpp>

#include <cstdio>
#include <fstream>
#include <iostream>
#include <string>
#include <vector>

using namespace radio_tool::codeplug;

constexpr auto CodeplugSize = 0x3000u;
constexpr auto TimestampOffset = 0x2001u;
constexpr auto GeneralOffset = 0x2040u;

static auto PutString(std::vector<uint8_t> &data, const size_t &offset, const std::string &str, const size_t &chars) -> void
{
    for (size_t ix = 0; ix < chars; ix++)
    {
        auto ch = ix < str.size() ? static_cast<uint16_t>(str[ix]) : 0xffff;
        data[offset + (ix * 2)] = ch & 0xff;
        data[offset + (ix * 2) + 1] = (ch >> 8) & 0xff;
    }
}

static auto MakeRDT(const std::string &radio, const std::vector<uint8_t> &timestamp, const size_t &truncate_to = 0) -> std::vector<uint8_t>
{
    std::vector<uint8_t> data(RDTHeaderSize + CodeplugSize, 0);

    const std::string magic = "DfuSe";
    std::copy(magic.begin(), magic.end(), data.begin());
    const std::string target = "Target";
    std::copy(target.begin(), target.end(), data.begin() + 0x0b);
    const std::string target_name = "MD-1701 codeplug";
    std::copy(target_name.begin(), target_name.end(), data.begin() + 0x16);
    std::copy(radio.begin(), radio.end(), data.begin() + 0x125);

    auto cp = RDTHeaderSize;
    std::copy(timestamp.begin(), timestamp.end(), data.begin() + cp + TimestampOffset);
    PutString(data, cp + GeneralOffset, "HELLO", 10);
    PutString(data, cp + GeneralOffset + 0x14, "WORLD", 10);
    PutString(data, cp + GeneralOffset + 0x70, "MYRADIO", 16);

    if (truncate_to != 0 && truncate_to < data.size())
    {
        data.resize(truncate_to);
    }
    return data;
}

static auto WriteTemp(const std::string &name, const std::vector<uint8_t> &data) -> std::string
{
    std::ofstream f(name, std::ios_base::out | std::ios_base::binary);
    f.write((const char *)data.data(), data.size());
    f.close();
    return name;
}

static auto Fail(const std::string &msg) -> int
{
    std::cerr << "FAIL: " << msg << std::endl;
    return 1;
}

auto main(int, char **) -> int
{
    auto err = 0;

    // valid codeplug
    {
        auto file = WriteTemp("test_codeplug_good.rdt", MakeRDT("DM-1701", {0x20, 0x21, 0x10, 0x26, 0x08, 0x56, 0x55}));
        try
        {
            auto h = CodeplugFactory::GetCodeplugHandler(file);
            h->Read(file);
            auto str = h->ToString();
            if (str.find("DM-1701") == std::string::npos)
                err |= Fail("radio model missing from output");
            if (str.find("2021") == std::string::npos)
                err |= Fail("timestamp missing from output");
            if (str.find("HELLO") == std::string::npos || str.find("WORLD") == std::string::npos)
                err |= Fail("intro lines missing from output");
            if (str.find("MYRADIO") == std::string::npos)
                err |= Fail("radio name missing from output");
        }
        catch (const std::exception &e)
        {
            err |= Fail(std::string("valid codeplug threw: ") + e.what());
        }
        std::remove(file.c_str());
    }

    // unsupported file must throw, not segfault
    {
        auto file = WriteTemp("test_codeplug_junk.rdt", std::vector<uint8_t>(1024, 0x41));
        try
        {
            auto h = CodeplugFactory::GetCodeplugHandler(file);
            h->Read(file);
            err |= Fail("unsupported file did not throw");
        }
        catch (const std::exception &)
        {
        }
        std::remove(file.c_str());
    }

    // missing file must throw
    {
        try
        {
            auto h = CodeplugFactory::GetCodeplugHandler("test_codeplug_does_not_exist.rdt");
            h->Read("test_codeplug_does_not_exist.rdt");
            err |= Fail("missing file did not throw");
        }
        catch (const std::exception &)
        {
        }
    }

    // truncated codeplug must throw, not read garbage
    {
        auto file = WriteTemp("test_codeplug_trunc.rdt", MakeRDT("DM-1701", {0x20, 0x21, 0x10, 0x26, 0x08, 0x56, 0x55}, RDTHeaderSize + 0x40));
        try
        {
            auto h = CodeplugFactory::GetCodeplugHandler(file);
            h->Read(file);
            err |= Fail("truncated codeplug did not throw");
        }
        catch (const std::exception &)
        {
        }
        std::remove(file.c_str());
    }

    // out of range timestamp must not crash
    {
        auto file = WriteTemp("test_codeplug_badts.rdt", MakeRDT("DM-1701", {0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff}));
        try
        {
            auto h = CodeplugFactory::GetCodeplugHandler(file);
            h->Read(file);
            (void)h->ToString();
        }
        catch (const std::exception &e)
        {
            err |= Fail(std::string("bad timestamp threw: ") + e.what());
        }
        std::remove(file.c_str());
    }

    return err;
}

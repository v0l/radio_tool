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
#include <radio_tool/dfu/tyt_dfu.hpp>
#include <radio_tool/dfu/dfu_exception.hpp>
#include <radio_tool/util.hpp>

#include <cstring>

using namespace radio_tool::dfu;

auto TYTDFU::IdentifyDevice() const -> std::string
{
    auto data = ReadRegister(TYTRegister::RadioInfo);

    //model is null-terminated str, get len with strlen
    auto slen = strlen((const char *)data.data());
    return std::string(data.begin(), data.begin() + slen + 1);
}

auto TYTDFU::ReadRegister(const TYTRegister &reg) const -> std::vector<uint8_t>
{
    Download({static_cast<uint8_t>(TYTDFU::RegisterCommand),
              static_cast<uint8_t>(reg)});

    return Upload(TYTDFU::RegisterSize);
}

auto TYTDFU::GetTime() const -> time_t
{
    InitUpload();
    auto time = ReadRegister(TYTRegister::RTC);

    return ParseBCDTimestamp(time.data());
}

auto TYTDFU::SetTime() const -> void
{
    Download({TYTDFU::CustomCommand,
              static_cast<uint8_t>(TYTCommand::SetRTC)});

    time_t rawtime;
    time(&rawtime);
    auto timeinfo = localtime(&rawtime);
    auto ts = MakeBCDTimestamp(*timeinfo);
    ts.insert(ts.begin(), 0xb5);

    Download(ts);
    //Reboot();
}

auto TYTDFU::Reboot() const -> void
{
    Download({TYTDFU::CustomCommand,
              static_cast<uint8_t>(TYTCommand::ProgrammingMode)});

    //this will normally throw an exception because the device
    //will not respond it will reboot immediately
    Download({TYTDFU::CustomCommand,
              static_cast<uint8_t>(TYTCommand::Reboot)});
}

auto TYTDFU::SendTYTCommand(const TYTCommand &cmd) const -> void
{
    Download({TYTDFU::CustomCommand,
              static_cast<uint8_t>(cmd)});
}
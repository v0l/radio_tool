/**
 * This file is part of radio_tool.
 * Copyright (c) 2022 v0l <radio_tool@v0l.io>
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
#include <radio_tool/radio/serial_radio_factory.hpp>
#include <radio_tool/radio/radio.hpp>

#include <functional>
#include <string>
#include <iostream>

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN 1
#include <Windows.h>
#else

#endif

using namespace radio_tool::radio;

auto SerialRadioFactory::GetRadioSupport(const uint16_t &idx) const -> const RadioOperations *
{
    return nullptr;
}

auto SerialRadioFactory::ListDevices(const uint16_t &idx_offset) const -> const std::vector<RadioInfo *>
{
    auto ret = std::vector<RadioInfo *>();

    OpDeviceList([&ret](const std::wstring &port, const uint16_t &idx)
                 { ret.push_back(new SerialRadioInfo(port, idx)); });
    return ret;
}

#ifdef _WIN32
auto SerialRadioFactory::OpDeviceList(std::function<void(const std::wstring &, const uint16_t &)> fn) const -> void
{
    HKEY comKey = nullptr;
    auto openResult = RegOpenKeyExW(HKEY_LOCAL_MACHINE, (LPWSTR)L"HARDWARE\\DEVICEMAP\\SERIALCOMM", 0, KEY_READ | KEY_WOW64_64KEY, &comKey);
    if (openResult != ERROR_SUCCESS)
    {
        throw std::runtime_error("Failed to enumerate serial ports");
    }

    constexpr auto BufferSize = 1024L;
    auto idx = 0UL;
    wchar_t name[BufferSize] = {}, value[BufferSize] = {};
    while (1)
    {
        auto nameSize = BufferSize;
        auto valueSize = BufferSize;
        memset(name, 0, BufferSize);
        memset(value, 0, BufferSize);

        auto readResult = RegEnumValueW(comKey, idx, name, (LPDWORD)&nameSize, NULL, NULL, (LPBYTE)value, (LPDWORD)&valueSize);
        if (readResult == ERROR_SUCCESS)
        {
            fn(std::wstring(value), (uint16_t)idx);
        }
        else if (readResult == ERROR_NO_MORE_ITEMS)
        {
            break;
        }
        else
        {
            throw std::runtime_error("Error reading registory key values");
        }
        idx++;
    }

    RegCloseKey(comKey);
}

#else
auto SerialRadioFactory::OpDeviceList(std::function<void(const std::string &, const uint16_t &)>) const -> void
{
}
#endif
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
#pragma once

#include <vector>
#include <stdint.h>
#include <iostream>
#include <sstream>
#include <chrono>
#include <algorithm>
#include <iterator>
#include <codecvt>

namespace radio_tool
{
    static auto PrintHex(std::vector<uint8_t>::const_iterator &&begin, std::vector<uint8_t>::const_iterator &&end) -> void
    {
        auto c = 1u;

        constexpr auto asciiZero = 0x30;
        constexpr auto asciiA = 0x61 - 0x0a;

        constexpr auto wordSize = 4;
        constexpr auto wordCount = 4;

        char eol_ascii[wordSize * wordCount + 1] = {};
        char aV, bV;
        std::stringstream prnt;
        auto size = std::distance(begin, end);
        while (begin != end)
        {
            auto v = (*begin);
            auto a = v & 0x0f;
            auto b = v >> 4;
            aV = (a <= 9 ? asciiZero : asciiA) + a;
            bV = (b <= 9 ? asciiZero : asciiA) + b;
            prnt << bV << aV << " ";

            auto col = c % (wordSize * wordCount);
            eol_ascii[col] = v >= 32 && v <= 127 ? (char)v : '.';
            if (col == 0 && c != size)
            {
                prnt << " " << eol_ascii << std::endl;
            }
            else if (c % wordSize == 0)
            {
                prnt << "  ";
            }
            if (c == size)
            {
                if (col > 0)
                {
                    eol_ascii[col + 1] = 0;
                }
                prnt << (col != 0 ? " " : "") << eol_ascii;
            }
            c++;
            std::advance(begin, 1);
        }

        std::cerr << prnt.str() << std::endl;
    }

    static constexpr auto _bcd(const uint8_t &c)
    {
        return (c & 0x0f) + ((c >> 4) * 10);
    }

    static constexpr auto _dcb(const uint8_t &c)
    {
        return ((int)(c / 10) * 16) + (c % 10);
    }

    static auto ParseBCDTimestamp(const uint8_t time[7]) -> time_t
    {
        std::tm t = {};
        t.tm_year = ((_bcd(time[0]) * 100) + _bcd(time[1])) - 1900;
        t.tm_mon = _bcd(time[2]) - 1;
        t.tm_mday = _bcd(time[3]);
        t.tm_hour = _bcd(time[4]);
        t.tm_min = _bcd(time[5]);
        t.tm_sec = _bcd(time[6]);

        return mktime(&t);
    }

    static auto MakeBCDTimestamp(const struct std::tm &timeinfo) -> std::vector<uint8_t>
    {
        return {
            static_cast<uint8_t>(_dcb((1900 + timeinfo.tm_year) / 100)),
            static_cast<uint8_t>(_dcb(timeinfo.tm_year + 1900 - (timeinfo.tm_year + 1900) / 100 * 100)),
            static_cast<uint8_t>(_dcb(timeinfo.tm_mon + 1)),
            static_cast<uint8_t>(_dcb(timeinfo.tm_mday)),
            static_cast<uint8_t>(_dcb(timeinfo.tm_hour)),
            static_cast<uint8_t>(_dcb(timeinfo.tm_min)),
            static_cast<uint8_t>(_dcb(timeinfo.tm_sec)),
        };
    }

    static inline auto ApplyXOR(std::vector<uint8_t> &data, const uint8_t *xor_key, const uint16_t &key_len) -> void
    {
        for (size_t z = 0; z < data.size(); z++)
        {
            data[z] = data[z] ^ xor_key[z % key_len];
        }
    }

    static inline auto ApplyXOR(std::vector<uint8_t>::iterator &&begin, std::vector<uint8_t>::iterator &&end, const uint8_t *xor_key, const uint16_t &key_len) -> void
    {
        auto z = 0;
        while (begin != end)
        {
            (*begin) = (*begin) ^ xor_key[z++ % key_len];
            std::advance(begin, 1);
        }
    }

    static auto BSDChecksum(std::vector<uint8_t>::iterator &data, const uint32_t &size) -> uint16_t
    {
        int32_t checksum = 0u;

        for (size_t i = 0; i < size; i++)
        {
            checksum = (checksum >> 1) + ((checksum & 1) << 15);
            checksum += *data;
            checksum &= 0xffff;
            std::advance(data, 1);
        }
        return checksum;
    }

    static auto Fletcher16(std::vector<uint8_t>::iterator &data, const uint32_t &size) -> uint16_t
    {
        constexpr auto block_size = 5802;
        uint32_t c0 = 0, c1 = 0;
        uint32_t i;
        int32_t len = size;

        // Found by solving for c1 overflow:
        // n > 0 and n * (n+1) / 2 * (2^8-1) < (2^32-1).
        for (c0 = c1 = 0; len > 0; len -= block_size)
        {
            uint32_t blocklen = std::min(block_size, len);
            for (i = 0; i < blocklen; ++i)
            {
                c0 = c0 + (*data);
                c1 = c1 + c0;
                std::advance(data, 1);
            }
            c0 = c0 % 255;
            c1 = c1 % 255;
        }
        //auto f0 = c0;
        //c0 = 0xff - ((c0 + c1) % 0xff);
        //c1 = 0xff - ((f0 + c0) % 0xff);
        return (c1 << 8 | c0);
    }

    static auto InternetChecksum(std::vector<uint8_t>::iterator &data, const uint32_t &size) -> uint16_t
    {
        int32_t sum = 0,
                count = size;

        // Main summing loop
        while (count > 1)
        {
            sum = sum + (*data);
            count = count - 2;
            std::advance(data, 1);
        }

        // Add left-over byte, if any
        if (count > 0)
        {
            sum = sum + (*data);
        }

        // Fold 32-bit sum to 16 bits
        while (sum >> 16)
        {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }

        return (~sum);
    }

    /**
     * Connect Systems checksum
     */
    static auto CSChecksum(std::vector<uint8_t>::const_iterator &&begin, const std::vector<uint8_t>::const_iterator &&end) -> uint16_t
    {
        uint16_t sum = 0;

        while (begin != end)
        {
            sum += (*begin);
            std::advance(begin, 1);
        }

        auto c0 = (int32_t)(sum / 5) >> 8;
        auto c1 = (sum / 5) & 0xff;
        return (c1 << 8 | c0);
    }

    static auto s2ws(const std::string &str) -> std::wstring
    {
        using convert_typeX = std::codecvt_utf8<wchar_t>;
        std::wstring_convert<convert_typeX, wchar_t> converterX;

        return converterX.from_bytes(str);
    }

    static auto ws2s(const std::wstring &wstr) -> std::string
    {
        using convert_typeX = std::codecvt_utf8<wchar_t>;
        std::wstring_convert<convert_typeX, wchar_t> converterX;

        return converterX.to_bytes(wstr);
    }
} // namespace radio_tool
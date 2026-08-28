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
#include <fstream>
#include <string>
#include <tuple>

#include <cstring>
#include <stdint.h>

using namespace std::literals::string_literals;

namespace radio_tool::codeplug
{
    const std::vector<std::tuple<std::string, uint32_t, uint32_t>> RadioConfigs = {
        /* Radio, Timestamp, General */
        { "DM-1701"s, 0x2001u, 0x2040u },
        { "2017"s, 0x2001u, 0x2040u },
        { "DR780"s, 0x2001u, 0x2040u }
    };

    enum class RDTType : uint8_t
    {
        Unknown = 0,
        TYT = 1,
        Anytone = 2
    };

    /**
     * Size of the RDT header in bytes
     */
    constexpr auto RDTHeaderSize = 0x225u;

    class RDTHeader 
    {
    public:
        RDTHeader()
            : type(RDTType::Unknown), n0(0), channel_offset(0), n1(0), n2(0), n3(0),
              n4(0), n5(0), n6(0), n7(0), nz(), complete(false)
        {
        }

        auto Read(std::ifstream& i) -> void 
        {
            complete = false;

            magic = ReadFixedString(i, 0x05);
            i.read((char*)&n0, sizeof(uint8_t));
            i.read((char*)&channel_offset, sizeof(uint32_t));
            i.read((char*)&n1, sizeof(uint8_t));
            target = ReadFixedString(i, 0x06);
            i.read((char*)&n2, sizeof(uint8_t));
            i.read((char*)&n3, sizeof(uint32_t));

            target_name = ReadFixedString(i, 0xff);

            i.read((char*)&n4, sizeof(uint32_t));
            i.read((char*)&n5, sizeof(uint32_t));
            i.read((char*)&n6, sizeof(uint32_t));
            i.read((char*)&n7, sizeof(uint32_t));

            radio = ReadFixedString(i, 0x10);

            i.read((char*)nz, sizeof(uint32_t) * 0x3c);

            //only mark the header as usable if every read above succeeded
            complete = i.good();
            type = complete ? RDTType::TYT : RDTType::Unknown;
        }

        auto Validate() const -> bool
        {
            if(!complete)
            {
                return false;
            }
            else if("DfuSe"s != magic)
            {
                return false;
            } 
            else if("Target"s != target)
            {
                return false;
            }
            return true;
        }

        auto GetTimestampOffset() const -> uint32_t
        {
            for(const auto& rx : RadioConfigs)
            {
                auto r = std::get<0>(rx);
                auto o = std::get<1>(rx);

                if(r == radio)
                {
                    return o;
                }
            }
            return 0x2001u; //default
        }
        
        auto GetGeneralOffset() const -> uint32_t
        {
            for(const auto& rx : RadioConfigs)
            {
                auto r = std::get<0>(rx);
                auto o = std::get<2>(rx);

                if(r == radio)
                {
                    return o;
                }
            }
            return 0x2040u; //default
        }

        RDTType type;
        std::string magic; //0x05
        uint8_t n0;
        uint32_t channel_offset; // +0x100
        uint8_t n1;
        std::string target; //0x06
        uint8_t n2;
        uint32_t n3;
        std::string target_name; // 0xff
        uint32_t n4;
        uint32_t n5;
        uint32_t n6;
        uint32_t n7;
        std::string radio; //0x10
        uint32_t nz[0x3c];

    private:
        /**
         * Read a fixed size string, truncated at the first null byte.
         * Unlike strlen this never reads past the end of the buffer when
         * the field is not null terminated.
         */
        static auto ReadFixedString(std::ifstream& i, const size_t& len) -> std::string
        {
            std::string ret(len, '\0');
            i.read((char*)ret.data(), len);
            if(!i.good())
            {
                return std::string();
            }
            auto end = ret.find('\0');
            if(end != std::string::npos)
            {
                ret.resize(end);
            }
            return ret;
        }

        bool complete;
    };
}
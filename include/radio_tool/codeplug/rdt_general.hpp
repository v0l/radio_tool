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

#include <fstream>
#include <string>
#include <sstream>
#include <vector>
#include <stdexcept>

#include <stdint.h>

namespace radio_tool::codeplug
{
    /**
     * Size of the general settings block in bytes
     */
    constexpr auto RDTGeneralSize = 0x90u;

    class RDTGeneral
    {
    public:
        RDTGeneral()
            : flags_1(0), flags_2(0), flags_3(0), radio_id(), tx_preamble(0),
              group_call_hang(0), private_call_hang(0), vox_level(0),
              rx_low_bat_interval(0), call_alert_tone_duration(0),
              lone_worker_response_time(0), lone_worker_reminder_time(0),
              scan_digital_hang_time(0), scan_analog_hang_time(0), flags_4(0),
              set_keypad_lock_time(0), mode(0), power_on_password(0),
              radio_prog_password(0), pc_prog_password()
        {
        }

        std::u16string intro_line1; //10
        std::u16string intro_line2; //10
        //skip 24
        uint8_t flags_1;
        uint8_t flags_2;
        uint8_t flags_3;
        //skip 1
        uint8_t radio_id[3];
        //skip 1
        uint8_t tx_preamble;
        uint8_t group_call_hang;
        uint8_t private_call_hang;
        uint8_t vox_level;
        //skip 2
        uint8_t rx_low_bat_interval;
        uint8_t call_alert_tone_duration;
        uint8_t lone_worker_response_time;
        uint8_t lone_worker_reminder_time;
        //skip 1
        uint8_t scan_digital_hang_time;
        uint8_t scan_analog_hang_time;
        uint8_t flags_4;
        uint8_t set_keypad_lock_time;
        uint8_t mode;
        uint32_t power_on_password;
        uint32_t radio_prog_password;
        uint8_t pc_prog_password[8];
        //skip 8
        std::u16string radio_name; //16

        auto Read(std::ifstream &i) -> void
        {
            intro_line1 = ReadString(i, 10);
            intro_line2 = ReadString(i, 10);
            i.seekg(24, std::ios_base::cur);
            i.read((char*)&flags_1, sizeof(uint8_t));
            i.read((char*)&flags_2, sizeof(uint8_t));
            i.read((char*)&flags_3, sizeof(uint8_t));
            i.seekg(1, std::ios_base::cur);
            i.read((char*)radio_id, sizeof(radio_id));
            i.seekg(1, std::ios_base::cur);
            i.read((char*)&tx_preamble, sizeof(uint8_t));
            i.read((char*)&group_call_hang, sizeof(uint8_t));
            i.read((char*)&private_call_hang, sizeof(uint8_t));
            i.read((char*)&vox_level, sizeof(uint8_t));
            i.seekg(2, std::ios_base::cur);
            i.read((char*)&rx_low_bat_interval, sizeof(uint8_t));
            i.read((char*)&call_alert_tone_duration, sizeof(uint8_t));
            i.read((char*)&lone_worker_response_time, sizeof(uint8_t));
            i.read((char*)&lone_worker_reminder_time, sizeof(uint8_t));
            i.seekg(1, std::ios_base::cur);
            i.read((char*)&scan_digital_hang_time, sizeof(uint8_t));
            i.read((char*)&scan_analog_hang_time, sizeof(uint8_t));
            i.read((char*)&flags_4, sizeof(uint8_t));
            i.read((char*)&set_keypad_lock_time, sizeof(uint8_t));
            i.read((char*)&mode, sizeof(uint8_t));
            i.read((char*)&power_on_password, sizeof(uint32_t));
            i.read((char*)&radio_prog_password, sizeof(uint32_t));
            i.read((char*)pc_prog_password, sizeof(pc_prog_password));
            i.seekg(8, std::ios_base::cur);
            radio_name = ReadString(i, 16);

            if(!i.good())
            {
                throw std::runtime_error("Codeplug is truncated, general settings block is incomplete");
            }
        }

        auto GetRadioId() const -> uint32_t
        {
            return radio_id[0] | (radio_id[1] << 8) | (radio_id[2] << 16);
        }

        auto ToString() const -> const std::string
        {
            std::stringstream out;

            out 
                << "Name:    " << ToUTF8(radio_name) << std::endl
                << "Radio ID: " << GetRadioId() << std::endl
                << "Intro 1: " << ToUTF8(intro_line1) << std::endl
                << "Intro 2: " << ToUTF8(intro_line2); //<< std::endl;

            return out.str();
        }

        /**
         * Convert a UTF-16LE codeplug string to UTF-8
         */
        static auto ToUTF8(const std::u16string &in) -> const std::string
        {
            std::string out;
            for (size_t ix = 0; ix < in.size(); ix++)
            {
                uint32_t cp = in[ix];
                //unused characters are padded with 0xffff
                if (cp == 0x0000 || cp == 0xffff)
                {
                    break;
                }
                if (cp >= 0xd800 && cp <= 0xdbff && (ix + 1) < in.size())
                {
                    //surrogate pair
                    uint32_t lo = in[ix + 1];
                    if (lo >= 0xdc00 && lo <= 0xdfff)
                    {
                        cp = 0x10000 + ((cp - 0xd800) << 10) + (lo - 0xdc00);
                        ix++;
                    }
                }
                if (cp < 0x80)
                {
                    out += static_cast<char>(cp);
                }
                else if (cp < 0x800)
                {
                    out += static_cast<char>(0xc0 | (cp >> 6));
                    out += static_cast<char>(0x80 | (cp & 0x3f));
                }
                else if (cp < 0x10000)
                {
                    out += static_cast<char>(0xe0 | (cp >> 12));
                    out += static_cast<char>(0x80 | ((cp >> 6) & 0x3f));
                    out += static_cast<char>(0x80 | (cp & 0x3f));
                }
                else
                {
                    out += static_cast<char>(0xf0 | (cp >> 18));
                    out += static_cast<char>(0x80 | ((cp >> 12) & 0x3f));
                    out += static_cast<char>(0x80 | ((cp >> 6) & 0x3f));
                    out += static_cast<char>(0x80 | (cp & 0x3f));
                }
            }
            return out;
        }

    private:
        /**
         * Read a fixed length UTF-16LE string
         */
        static auto ReadString(std::ifstream &i, const size_t &chars) -> std::u16string
        {
            std::vector<uint8_t> buf(chars * 2, 0);
            i.read((char *)buf.data(), buf.size());
            if (!i.good())
            {
                return std::u16string();
            }

            std::u16string ret;
            ret.reserve(chars);
            for (size_t ix = 0; ix < chars; ix++)
            {
                ret.push_back(static_cast<char16_t>(buf[ix * 2] | (buf[(ix * 2) + 1] << 8)));
            }
            return ret;
        }
    };
}
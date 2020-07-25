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

#include <fstream>
#include <string>
#include <sstream>

#include <stdint.h>

namespace radio_tool::codeplug
{
    class RDTGeneral
    {
    public:
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
            intro_line1.reserve(10);
            intro_line2.reserve(10);
            i.read((char*)intro_line1.data(), sizeof(char16_t) * 10);
            i.read((char*)intro_line2.data(), sizeof(char16_t) * 10);
            intro_line1.shrink_to_fit();
            intro_line2.shrink_to_fit();
        }

        auto ToString() const -> const std::u16string
        {
            std::basic_stringstream<char16_t> out;

            out 
                << "Intro 1: " << intro_line1 << std::endl
                << "Intro 2: " << intro_line2; //<< std::endl;

            return out.str();
        }
    };
}
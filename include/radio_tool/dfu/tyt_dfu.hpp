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

#include <radio_tool/dfu/dfu.hpp>

namespace radio_tool::dfu
{
    /**
     * Custom radio commands starting with 0x91
     */
    enum class TYTCommand : uint8_t {
        ProgrammingMode = 0x01,
        SetRTC = 0x02,
        Reboot = 0x05
    };

    enum class TYTRegister : uint8_t {
        RadioInfo = 0x01, //Radio Model(16 bytes) + 16 bytes of something else
        R_02 = 0x02, //unknown (4 bytes)
        R_03 = 0x03, //unknown (24 bytes)
        R_04 = 0x04, //unknown (8 bytes)
        R_07 = 0x07, //unknown (16 bytes)
        RTC = 0x08, //Real time clock (7 bytes)
    };

    class TYTDFU : public DFU
    {
    public:
        static const auto VID = 0x0483;
        static const auto PID = 0xdf11;

        static const auto CustomCommand = 0x91;
        static const auto RegisterCommand = 0xa2;
        static const auto RegisterSize = 1024;

        TYTDFU() : DFU(VID, PID, idx) { }
        auto IdentifyDevice() const -> std::string;
        auto ReadRegister(const TYTRegister& reg) const -> std::vector<uint8_t>;
        auto SendCustom(const std::vector<uint8_t>& data) const -> void;

        /**
         * Gets the current time from the radio's RTC
         * This doesn't work during TX/RX
         */
        auto GetTime() const -> const time_t;

        /**
         * Set the time on the radio to the current machine time
         */
        auto SetTime() const -> void;

        /**
         * Reboot the device
         */
        auto Reboot() const -> void;
    protected:
        /**
         * Ensures the state is DFU_IDLE or DFU_DNLOAD_IDLE
         */
        auto InitDownload() const -> void;

        /**
         * Ensures the state is DFU_IDLE or DFU_DPLOAD_IDLE
         */
        auto InitUpload() const -> void;
    };
} // namespace radio_tool::dfu
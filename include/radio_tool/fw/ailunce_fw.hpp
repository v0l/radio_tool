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

#include <radio_tool/fw/fw.hpp>
#include <radio_tool/fw/cipher/uv3x0.hpp>
#include <radio_tool/fw/cipher/dm1701.hpp>
#include <radio_tool/fw/cipher/md380.hpp>
#include <radio_tool/fw/cipher/md9600.hpp>

#include <fstream>
#include <cstring>
#include <sstream>
#include <iomanip>

namespace radio_tool::fw
{

    class AilunceFW : public FirmwareSupport
    {
    public:
        AilunceFW() {}

        auto Read(const std::string &file) -> void override;
        auto Write(const std::string &file) -> void override;
        auto ToString() const -> std::string override;
        auto Decrypt() -> void override;
        auto Encrypt() -> void override;
        auto SetRadioModel(const std::string&) -> void override;

        /**
         * @note This is not the "firmware_model" which exists in the firmware header
         */
        auto GetRadioModel() const -> const std::string override;

        /**
         * Tests a file if its a valid firmware file
         */
        static auto SupportsFirmwareFile(const std::string &file) -> bool;

        /**
         * Tests if a radio model is supported by this firmware handler
         */
        static auto SupportsRadioModel(const std::string &model) -> bool;

        /**
         * Create an instance of this class for the firmware factory
         */
        static auto Create() -> std::unique_ptr<FirmwareSupport>
        {
            return std::make_unique<AilunceFW>();
        }

    private:
        std::string radio_model;
        auto ApplyXOR() -> void;

    };

} // namespace radio_tool::fw

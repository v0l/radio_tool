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

#include <vector>
#include <string>

#include <stdint.h>

namespace radio_tool::codeplug
{
    class CodeplugSupport
    {
    public:
        /**
         * Read a codeplug from disk
         */
        virtual auto Read(const std::string&) -> void = 0;

        /**
         * Write a codeplug to disk
         */
        virtual auto Write(const std::string&) const -> void = 0;

        /**
         * Get the codeplug data to write to a device
         */
        virtual auto GetData() const -> const std::vector<uint8_t> = 0;

        /**
         * Get some general info about the loaded codeplug
         */
        virtual auto ToString() const -> const std::string = 0;
    };
}
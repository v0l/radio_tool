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
#include <radio_tool/fw/tyt_fw.hpp>

#include <string>
#include <memory>
#include <functional>

namespace radio_tool::fw
{
    /**
     * A list of functions to test each firmware handler,
     * and a function to create a new instance of the handler
     */
    const std::vector<std::pair<std::function<bool(const std::string &)>, std::function<std::unique_ptr<FirmwareSupport>()>>> FirmwareSupports = {
        {TYTFW::SupportsFirmwareFile, TYTFW::Create}
    };

    class FirmwareFactory
    {
    public:
        /**
         * Return a handler for the firmware file
         * @note Normally used for firmware only operations
         */
        static auto GetFirmwareHandler(const std::string &) -> std::unique_ptr<FirmwareSupport>;
    };
} // namespace radio_tool::fw
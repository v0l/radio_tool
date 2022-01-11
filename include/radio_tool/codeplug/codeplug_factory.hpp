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

#include <radio_tool/codeplug/codeplug.hpp>
#include <radio_tool/codeplug/rdt.hpp>

#include <string>
#include <memory>
#include <functional>

namespace radio_tool::codeplug
{
    /**
     * All codeplug handlers
     */
    const std::vector<std::pair<std::function<bool(const std::string &)>, std::function<std::unique_ptr<CodeplugSupport>()>>> AllCodeplugs = {
        {RDT::SupportsCodeplug, RDT::Create}
    };

    class CodeplugFactory
    {
    public:
        static auto GetCodeplugHandler(const std::string &file) -> std::unique_ptr<CodeplugSupport>
        {
            for (const auto &try_this : AllCodeplugs)
            {
                if (try_this.first(file))
                {
                    return try_this.second();
                }
            }
        }
    };
} // namespace radio_tool::codeplug
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

#include <radio_tool/fw/fw.hpp>
#include <radio_tool/fw/tyt_fw.hpp>
#include <radio_tool/fw/cs_fw.hpp>
#include <radio_tool/fw/ailunce_fw.hpp>

#include <string>
#include <memory>
#include <functional>

namespace radio_tool::fw
{
    class FirmwareSupportTest
    {
    public:
        FirmwareSupportTest(
            std::function<bool(const std::string &)> &&fnFile,
            std::function<bool(const std::string &)> &&fnRadio,
            std::function<std::unique_ptr<FirmwareSupport>()> &&fnCreate
        ) : SupportsRadioModel(fnRadio), SupportsFirmwareFile(fnFile), CreateHandler(fnCreate)
        {

        }
        const std::function<bool(const std::string &)> SupportsFirmwareFile;

        const std::function<bool(const std::string &)> SupportsRadioModel;

        const std::function<std::unique_ptr<FirmwareSupport>()> CreateHandler;
    };

    /**
     * All firmware handlers
     */
    const std::vector<FirmwareSupportTest> AllFirmwareHandlers = {
        FirmwareSupportTest(TYTFW::SupportsFirmwareFile, TYTFW::SupportsRadioModel, TYTFW::Create),
        FirmwareSupportTest(CSFW::SupportsFirmwareFile, CSFW::SupportsRadioModel, CSFW::Create),
        FirmwareSupportTest(AilunceFW::SupportsFirmwareFile, AilunceFW::SupportsRadioModel, AilunceFW::Create)
    };

    class FirmwareFactory
    {
    public:
        /**
         * Return a handler for the firmware file
         * @note Normally used for firmware only operations
         */
        static auto GetFirmwareFileHandler(const std::string &file) -> std::unique_ptr<FirmwareSupport>
        {
            for (const auto &fn : AllFirmwareHandlers)
            {
                if (fn.SupportsFirmwareFile(file))
                {
                    return fn.CreateHandler();
                }
            }
            throw std::runtime_error("Firmware file not supported");
        }

        /**
         * Return a handler for the firmware file
         * @note Normally used for firmware only operations
         */
        static auto GetFirmwareModelHandler(const std::string &model) -> std::unique_ptr<FirmwareSupport>
        {
            for (const auto &fn : AllFirmwareHandlers)
            {
                if (fn.SupportsRadioModel(model))
                {
                    return fn.CreateHandler();
                }
            }
            throw std::runtime_error("Firmware model not supported");
        }
    };
} // namespace radio_tool::fw
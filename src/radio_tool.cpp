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
#include <radio_tool/radio/radio_factory.hpp>
#include <radio_tool/fw/fw_factory.hpp>

#include <radio_tool/dfu/dfu_exception.hpp>
#include <radio_tool/util.hpp>

#include <iostream>
#include <filesystem>
#include <cxxopts.hpp>

using namespace radio_tool::fw;
using namespace radio_tool::radio;

int main(int argc, char **argv)
{
    try
    {
        cxxopts::Options options(argv[0]);

        options.add_options("General")
            ("h,help", "Show this message")
            ("list", "List devices")
            ("d,device", "Device to use", cxxopts::value<uint16_t>(), "<index>");

        options.add_options("Programming")
            ("f,flash", "Flash firmware", cxxopts::value<std::string>(), "<firmware.bin>")
            ("p,program", "Upload codeplug", cxxopts::value<std::string>(), "<codeplug.rtd>")
            ("get-time", "Gets the radio time")
            ("set-time", "Sets the radio time")
            ("dump-reg", "Dump a register from the radio", cxxopts::value<uint16_t>(), "<register>")
            ("write-custom", "Send custom command to radio", cxxopts::value<std::vector<uint8_t>>(), "<data>")
            ("get-status", "Return the current DFU Status")
            ("reboot", "Reboot the radio");

        options.add_options("Firmware")
            ("info", "Return info about a firmware file", cxxopts::value<std::string>(), "<firmware.bin>")
            ("wrap", "Wrap a firmware bin", cxxopts::value<std::string>(), "<firmware.bin>")
            ("unwrap", "Unwrap a fimrware file", cxxopts::value<std::string>(), "<firmware.bin>");

        auto cmd = options.parse(argc, argv);

        if (cmd.count("help") || cmd.arguments().empty())
        {
            std::cerr << options.help({"General", "Programming", "Firmware"}) << std::endl;
            exit(0);
        }

        //do non device specific commands
        if (cmd.count("info"))
        {
            auto file = cmd["info"].as<std::string>();
            
            auto fwFact = FirmwareFactory();
            auto fw = fwFact.GetFirmwareHandler(file);
            fw->Read(file);
            std::cerr << fw->ToString();
            exit(0);
        }
        
        auto rdFactory = RadioFactory();
        if (cmd.count("list"))
        {
            for (const auto &d : rdFactory.ListDevices())
            {
                std::wcout << d.ToString() << std::endl;
            }
            exit(0);
        }

        if (!cmd.count("device"))
        {
            std::wcout << L"You must select a device" << std::endl;
            exit(1);
        }


        auto index = cmd["device"].as<uint16_t>();
        auto radio = rdFactory.GetRadioSupport(index);

        if(cmd.count("flash")) {
            auto file = cmd["flags"].as<std::string>();
            radio->WriteFirmware(file);
        }

        if(cmd.count("program")) {

        }

        if(cmd.count("dump-reg")) 
        {
            auto x = cmd["dump-reg"].as<uint16_t>();
            std::cerr << "Read register: 0x" << std::setfill('0') << std::setw(2) << std::hex << x << std::endl;
            //radio_tool::PrintHex(dfu.ReadRegister(static_cast<const TYTRegister>(x)));
        }

        if(cmd.count("write-custom")) {
            auto data = cmd["write-custom"].as<std::vector<uint8_t>>();
            //dfu.SendCustom(data);
        }

        if(cmd.count("get-status")) {
            //auto status = dfu.GetStatus();
            //std::cerr << status.ToString() << std::endl;
        }

        if(cmd.count("get-time")) {
            //auto tm = dfu.GetTime();
            //std::cerr << ctime(&tm);
        }

        if(cmd.count("set-time")) {
            //dfu.SetTime();
        }

        if(cmd.count("reboot")){
            //dfu.Reboot();
        }
    }
    catch (const radio_tool::dfu::DFUException& dfuEx) 
    {
        std::cout << "Error: " << dfuEx.what() << std::endl;
    }
    catch (const cxxopts::OptionException &e)
    {
        std::cout << "error parsing options: " << e.what() << std::endl;
        exit(1);
    }
}

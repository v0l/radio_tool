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

#ifdef XOR_TOOL
#include <xor_tool.hpp>
#endif

#include <iostream>
#include <filesystem>
#include <cxxopts.hpp>
#include <fstream>

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
            ("d,device", "Device to use", cxxopts::value<uint16_t>(), "<index>")
            ("i,in", "Input file", cxxopts::value<std::string>(), "<file>")
            ("o,out", "Output file", cxxopts::value<std::string>(), "<file>");

        options.add_options("Programming")
            ("f,flash", "Flash firmware")
            ("p,program", "Upload codeplug");
        
        options.add_options("All radio")
            ("info", "Print some info about the radio")
            ("write-custom", "Send custom command to radio", cxxopts::value<std::vector<uint8_t>>(), "<data>")
            ("get-status", "Return the current DFU Status");

        options.add_options("TYT Radio")
            ("get-time", "Gets the radio time")
            ("set-time", "Sets the radio time")
            ("dump-reg", "Dump a register from the radio", cxxopts::value<uint16_t>(), "<register>")
            ("reboot", "Reboot the radio")
            ("dump-bootloader", "Dump bootloader (Mac only)");

        options.add_options("Firmware")
            ("fw-info", "Return info about a firmware file")
            ("wrap", "Wrap a firmware bin")
#ifdef XOR_TOOL
            ("make-xor", "Try to make an XOR key for the input firmware")        
#endif
            ("unwrap", "Unwrap a fimrware file");


        auto cmd = options.parse(argc, argv);

        if (cmd.count("help") || cmd.arguments().empty())
        {
            std::cerr << options.help({"General", "Programming", "Firmware", "All radio", "TYT Radio"}) << std::endl;
            exit(0);
        }

        //do non device specific commands
        if (cmd.count("fw-info"))
        {
            if(cmd.count("in")) 
            {
                auto file = cmd["in"].as<std::string>();
                
                auto fw = FirmwareFactory::GetFirmwareHandler(file);
                fw->Read(file);
                std::cerr << fw->ToString();
                exit(0);
            } 
            else 
            {
                std::cerr << "Input file not specified" << std::endl;
                exit(1);
            }
        }

        if(cmd.count("unwrap")) 
        {
            if(cmd.count("in") && cmd.count("out")) 
            {
                auto in_file = cmd["in"].as<std::string>();
                auto out_file = cmd["out"].as<std::string>();
                
                auto fw_handler = FirmwareFactory::GetFirmwareHandler(in_file);
                fw_handler->Read(in_file);
                fw_handler->Decrypt();

                auto r_offset = 0;
                for(const auto& rn : fw_handler->GetMemoryRanges()) 
                {
                    std::stringstream ss_name;
                    ss_name << out_file << "_0x" << std::setw(8) << std::setfill('0') << std::hex << rn.first;

                    std::ofstream fw_out;
                    fw_out.open(ss_name.str(), std::ios_base::out | std::ios_base::binary);
                    fw_out.exceptions(std::ios_base::badbit);
                    if(fw_out.is_open()) 
                    {
                        auto data = fw_handler->GetData();
                        fw_out.write((const char*)data.data() + r_offset, rn.second);
                        fw_out.close();
                    } 
                    else 
                    {
                        std::cerr << "Failed to open output file: " << out_file << std::endl;
                        exit(1);
                    }
                    r_offset += rn.second;
                }
                exit(0);
            }
            else 
            {
                std::cerr << "Input/Output file not specified" << std::endl;
                exit(1);
            }
        }

#ifdef XOR_TOOL
        if(cmd.count("make-xor")) 
        {
            if(cmd.count("in")) 
            {
                auto in_file = cmd["in"].as<std::string>();                
                auto fw_handler = FirmwareFactory::GetFirmwareHandler(in_file);
                fw_handler->Read(in_file);
                
                auto data = fw_handler->GetData();
                radio_tool::PrintHex(radio_tool::fw::XORTool::MakeXOR(data));
                exit(0);
            }
            else 
            {
                std::cerr << "Input/Output file not specified" << std::endl;
                exit(1);
            }
        }
#endif
        
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
            std::cout << "You must select a device" << std::endl;
            exit(1);
        }

        auto index = cmd["device"].as<uint16_t>();
        auto radio = rdFactory.GetRadioSupport(index);
        auto dfu = radio->GetDFU();
        
        if(cmd.count("info")) 
        {
            std::cout << radio->ToString() << std::endl;
            exit(1);
        }

        if(cmd.count("flash")) 
        {
            if(cmd.count("in")) 
            {
                auto file = cmd["in"].as<std::string>();
                radio->WriteFirmware(file);
            } 
            else 
            {
                std::cerr << "Input file not specified" << std::endl;
                exit(1);
            }
        }

        if(cmd.count("program")) 
        {

        }

        if(cmd.count("dump-reg")) 
        {
            auto x = cmd["dump-reg"].as<uint16_t>();
            std::cerr << "Read register: 0x" << std::setfill('0') << std::setw(2) << std::hex << x << std::endl;
            //radio_tool::PrintHex(dfu.ReadRegister(static_cast<const TYTRegister>(x)));
        }

        if(cmd.count("dump-bootloader")) 
        {
            if(cmd.count("out")) 
            {
                auto file = cmd["out"].as<std::string>();
                auto size = 0xc000;
                std::ofstream outf;
                outf.open(file, std::ios_base::out | std::ios_base::binary);
                if(outf.is_open()) 
                {
                    auto mem = dfu.Upload(size, 2);
                    //radio_tool::PrintHex(mem);
                    outf.write((char*)mem.data(), mem.size());
                    outf.close();
                }
                else 
                {
                    std::cerr << "Failed to open output file: " << file << std::endl;
                    exit(1);
                }
            }
            else 
            {
                std::cerr << "Output file not specified" << std::endl;
                exit(1);
            }
        }


        if(cmd.count("write-custom")) 
        {
            auto data = cmd["write-custom"].as<std::vector<uint8_t>>();
            dfu.Download(data);
        }

        if(cmd.count("get-status")) 
        {
            auto status = dfu.GetStatus();
            std::cerr << status.ToString() << std::endl;
        }

        if(cmd.count("get-time")) 
        {
            //auto tm = dfu.GetTime();
            //std::cerr << ctime(&tm);
        }

        if(cmd.count("set-time")) 
        {
            //dfu.SetTime();
        }

        if(cmd.count("reboot"))
        {
            //dfu.Reboot();
        }
    }
    catch (const radio_tool::dfu::DFUException& dfuEx) 
    {
        std::cerr << "DFU Error: " << dfuEx.what() << std::endl;
         exit(1);
    }
    catch (const cxxopts::OptionException &e)
    {
        std::cerr << "error parsing options: " << e.what() << std::endl;
        exit(1);
    }
    catch (const std::exception &gex) 
    {
        std::cerr << "Error: " << gex.what() << std::endl;
         exit(1);
    }
}

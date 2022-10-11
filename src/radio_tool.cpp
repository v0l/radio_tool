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
#include <radio_tool/radio/radio_factory.hpp>
#include <radio_tool/fw/fw_factory.hpp>
#include <radio_tool/codeplug/codeplug_factory.hpp>

#include <radio_tool/radio/tyt_radio.hpp>
#include <radio_tool/dfu/dfu_exception.hpp>
#include <radio_tool/util.hpp>
#include <radio_tool/version.hpp>

#ifdef XOR_TOOL
#include <xor_tool.hpp>
#endif

#include <iostream>
#include <filesystem>
#include <cxxopts.hpp>
#include <fstream>

using namespace radio_tool::fw;
using namespace radio_tool::radio;
using namespace radio_tool::codeplug;

auto tytCommands(const cxxopts::ParseResult &cmd, const RadioOperations *radio) -> void;

template <class T>
auto GetOptionOrErr(const cxxopts::ParseResult &cmd, const std::string &v, const std::string &err) -> const T &
{
    if (cmd.count(v))
    {
        return cmd[v].as<T>();
    }
    else
    {
        throw std::runtime_error(err);
    }
}

int main(int argc, char **argv)
{
    try
    {
        std::stringstream ssVersion;
        ssVersion << g_PROJECT_NAME << " v" << g_PROJECT_VERSION << "-" << g_GIT_SHA1_SHORT;
        auto version = ssVersion.str();

        cxxopts::Options options(argv[0], version);

        options.add_options("General")
            ("h,help", "Show this message", cxxopts::value<std::string>()->implicit_value(""), "<command>")
            ("l,list", "List devices")
            ("d,device", "Device to use", cxxopts::value<uint16_t>(), "<index>")
            ("i,in", "Input file", cxxopts::value<std::string>(), "<file>")
            ("o,out", "Output file", cxxopts::value<std::string>(), "<file>")
            ("L,list-radios", "List supported radios");

        options.add_options("Programming")
            ("f,flash", "Flash firmware")
            ("p,program", "Upload codeplug")
            ("dump-firmware", "Download firmware backup", cxxopts::value<uint16_t>(), "<size>");

        options.add_options("All radio")
            ("info", "Print some info about the radio")
            ("write-custom", "Send custom command to radio", cxxopts::value<std::vector<uint8_t>>(), "<data>")
            ("get-status", "Print the current DFU Status");

        options.add_options("TYT Radio")
            ("get-time", "Gets the radio time")
            ("set-time", "Sets the radio time")
            ("reboot", "Reboot the radio")
            ("dump-bootloader", "Dump bootloader (Mac only)");

        options.add_options("Firmware")
            ("fw-info", "Print info about a firmware file")
            ("wrap", "Wrap a firmware bin (use --help wrap, for more info)")
            ("unwrap", "Unwrap a fimrware file")
#ifdef XOR_TOOL
            ("make-xor", "Try to make an XOR key for the input firmware");
#else
            ;
#endif

        options.add_options("Codeplug")
            ("codeplug-info", "Print info about a codeplug file");

        options.add_options("Wrap")
            ("s,segment", "Add a segment for wrapping", cxxopts::value<std::vector<std::string>>(), "<0x08000000:region_0.bin>")
            ("r,radio", "Radio to build firmware file for", cxxopts::value<std::string>(), "<DM1701>");

        auto cmd = options.parse(argc, argv);

        if (cmd.count("help") || cmd.arguments().empty())
        {
            std::vector<std::string> help_groups;

            auto section = cmd.count("help") ? cmd["help"].as<std::string>() : std::string();
            if (!section.empty())
            {
                for (const auto &g : options.groups())
                {
                    if (g.size() != section.size())
                        continue;
                    if (std::equal(g.begin(), g.end(), section.begin(), [](char a, char b)
                                   { return std::toupper(a) == std::toupper(b); }))
                    {
                        help_groups.push_back(g);
                    }
                }
            }
            else
            {
                help_groups = {"General", "Programming", "Firmware", "All radio", "TYT Radio", "Codeplug"};
            }
            std::cerr << options.help(help_groups) << std::endl;
            exit(0);
        }

        if (cmd.count("list-radios"))
        {
            //TODO: list radio models supported
            exit(0);
        }

        //do non device specific commands
        if (cmd.count("fw-info"))
        {
            auto file = GetOptionOrErr<std::string>(cmd, "in", "Input file not specified");

            auto fw = FirmwareFactory::GetFirmwareFileHandler(file);
            fw->Read(file);
            std::cerr << fw->ToString();
            exit(0);
        }

        if (cmd.count("codeplug-info"))
        {
            auto file = GetOptionOrErr<std::string>(cmd, "in", "Input file not specified");

            auto h = CodeplugFactory::GetCodeplugHandler(file);
            h->Read(file);
            std::cerr << h->ToString();
            exit(0);
        }

        if (cmd.count("wrap"))
        {
            auto out = GetOptionOrErr<std::string>(cmd, "out", "Output file not specified");
            auto radio = GetOptionOrErr<std::string>(cmd, "radio", "Radio not specified");
            auto segments = GetOptionOrErr<std::vector<std::string>>(cmd, "segment", "Must specify at least 1 segment");

            auto fw = FirmwareFactory::GetFirmwareModelHandler(radio);
            fw->SetRadioModel(radio);
            for (const auto &sx : segments)
            {
                auto schar = sx.find(':');
                if (schar != sx.npos)
                {
                    uint32_t addr = 0;
                    auto addr_str = sx.substr(0, schar);
                    if (addr_str.find("0x") != addr_str.npos)
                    {
                        addr = std::stoi(addr_str, 0, 16);
                    }
                    else
                    {
                        addr = std::stoi(addr_str, 0, 10);
                    }

                    auto filename = sx.substr(schar + 1);
                    std::cerr << "Adding segment 0x"
                              << std::hex << std::setw(8) << std::setfill('0') << addr
                              << " from file " << filename << std::endl;

                    std::ifstream f_seg(filename, std::ios_base::binary);
                    if (f_seg.is_open())
                    {
                        f_seg.seekg(0, f_seg.end);
                        auto len = f_seg.tellg();
                        f_seg.seekg(0, f_seg.beg);

                        std::vector<uint8_t> seg_data;
                        seg_data.resize(len);
                        f_seg.read((char *)seg_data.data(), len);
                        f_seg.close();

                        fw->AppendSegment(addr, seg_data);
                    }
                    else
                    {
                        throw std::runtime_error("Cant open file for segment");
                    }
                }
                else
                {
                    throw std::invalid_argument("Segments must be in the format '0x0000:filename.bin'");
                }
            }

            fw->Encrypt();
            fw->Write(out);
            std::cerr << "Done!" << std::endl;
            exit(0);
        }

        if (cmd.count("unwrap"))
        {
            auto in_file = GetOptionOrErr<std::string>(cmd, "in", "Input file not specified");
            auto out_file = GetOptionOrErr<std::string>(cmd, "out", "Output file not specified");

            auto fw_handler = FirmwareFactory::GetFirmwareFileHandler(in_file);
            fw_handler->Read(in_file);
            fw_handler->Decrypt();

            for (const auto &rn : fw_handler->GetDataSegments())
            {
                std::stringstream ss_name;
                ss_name << out_file << "_0x" << std::setw(8) << std::setfill('0') << std::hex << rn.address;

                std::ofstream fw_out;
                fw_out.open(ss_name.str(), std::ios_base::out | std::ios_base::binary);
                if (fw_out.is_open())
                {
                    fw_out.write((const char *)rn.data.data(), rn.data.size());
                    fw_out.close();
                }
                else
                {
                    std::cerr << "Failed to open output file: " << out_file << std::endl;
                    exit(1);
                }
            }
            exit(0);
        }

#ifdef XOR_TOOL
        if (cmd.count("make-xor"))
        {
            auto in_file = GetOptionOrErr<std::string>(cmd, "in", "Input file not specified");
            auto fw_handler = FirmwareFactory::GetFirmwareFileHandler(in_file);
            fw_handler->Read(in_file);

            auto key = radio_tool::fw::XORTool::MakeXOR(fw_handler->GetData());
            for (const auto &region : fw_handler->GetDataSegments())
            {
                if (radio_tool::fw::XORTool::Verify(region.address, region.data, key))
                {
                    std::cout
                        << "Region @ 0x" << std::setfill('0') << std::setw(8) << std::hex << region.address
                        << " appears to be a valid vector_table" << std::endl;
                }
            }

            radio_tool::PrintHex(key.begin(), key.end());

            // dump in C format
            std::cout << "const uint8_t[] key = {";
            for(const auto &v : key) {
                std::cout << "0x" << std::hex << std::setfill('0') << std::setw(2) << (int)v << ",";
            }
            std::cout << "};" << std::endl;
            exit(0);
        }
#endif

        auto rdFactory = RadioFactory();
        if (cmd.count("list"))
        {
            for (const auto &d : rdFactory.ListDevices())
            {
                std::wcout << d->ToString() << std::endl;
            }
            exit(0);
        }

        if (!cmd.count("device"))
        {
            std::cout << "You must select a device" << std::endl;
            exit(1);
        }

        auto index = cmd["device"].as<uint16_t>();
        auto radio = rdFactory.OpenDevice(index);
        auto device = radio->GetDevice();

        if (cmd.count("info"))
        {
            std::cout << radio->ToString() << std::endl;
            exit(1);
        }

        if (cmd.count("flash"))
        {
            auto in_file = GetOptionOrErr<std::string>(cmd, "in", "Input file not specified");
            radio->WriteFirmware(in_file);
            std::cout << "Done!" << std::endl;
        }

        if (cmd.count("program"))
        {
        }

        if (cmd.count("write-custom"))
        {
            auto data = cmd["write-custom"].as<std::vector<uint8_t>>();
            device->Write(data);
        }

        tytCommands(cmd, radio);
    }
    catch (const radio_tool::dfu::DFUException &dfuEx)
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

auto tytCommands(const cxxopts::ParseResult &cmd, const RadioOperations *radio) -> void
{
    if (typeid(radio) == typeid(radio_tool::radio::TYTRadio))
    {
        std::cerr << "Cant use TYT commands on non-tyt radio!" << std::endl;
        exit(1);
    }

    auto tyt_radio = dynamic_cast<radio_tool::radio::TYTRadio *>(const_cast<RadioOperations*>(radio));
    auto device = tyt_radio->GetDevice();
    auto dfu = device->GetDFU();

    if (cmd.count("get-status"))
    {
        auto status = device->Status();
        std::cerr << status << std::endl;
    }

    if (cmd.count("dump-bootloader"))
    {
        auto out_file = GetOptionOrErr<std::string>(cmd, "out", "Output file not specified");
        auto size = 0xc000;
        std::ofstream outf;
        outf.open(out_file, std::ios_base::out | std::ios_base::binary);
        if (outf.is_open())
        {
            auto mem = dfu.Upload(size, 2);
            //radio_tool::PrintHex(mem);
            outf.write((char *)mem.data(), mem.size());
            outf.close();
        }
        else
        {
            std::cerr << "Failed to open output file: " << out_file << std::endl;
            exit(1);
        }
    }

    if (cmd.count("dump-firmware"))
    {
        auto out_file = GetOptionOrErr<std::string>(cmd, "out", "Output file not specified");
        std::ofstream outf;
        outf.open(out_file, std::ios_base::out | std::ios_base::binary);
        if (outf.is_open())
        {
            dfu.SendTYTCommand(radio_tool::dfu::TYTCommand::FirmwareUpgrade);
            auto mem = dfu.Upload(0x4000, 2);
            //radio_tool::PrintHex(mem.begin(), mem.end());
            outf.write((char *)mem.data(), mem.size());
            outf.close();
        }
        else
        {
            std::cerr << "Failed to open output file: " << out_file << std::endl;
            exit(1);
        }
    }

    if (cmd.count("get-time"))
    {
        //auto tm = dfu.GetTime();
        //std::cerr << ctime(&tm);
    }

    if (cmd.count("set-time"))
    {
        //dfu.SetTime();
    }

    if (cmd.count("reboot"))
    {
        //dfu.Reboot();
    }
}
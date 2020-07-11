#include <tyt_tool/tyt_dfu.hpp>
#include <tyt_tool/tyt_fw.hpp>
#include <tyt_tool/dfu_exception.hpp>
#include <tyt_tool/util.hpp>

#include <iostream>
#include <filesystem>
#include <cxxopts.hpp>

using namespace tyt_tool::dfu;
using namespace tyt_tool::fw;

auto constexpr VID = 0x0483;
auto constexpr PID = 0xdf11;

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
            ("dump-reg", "Dump a register from the radio", cxxopts::value<uint16_t>(), "<register>");

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

        auto dfu = new TYT(VID, PID);
        if (!dfu->Init())
        {
            std::wcout << L"No devices found" << std::endl;
        }
        else
        {
            if (cmd.count("list"))
            {
                for (const auto &d : dfu->ListDevices())
                {
                    std::wcout << d << std::endl;
                }
                exit(0);
            }

            //do non device specific commands
            if (cmd.count("info"))
            {
                const char magic[] = {'\x01', '\x0f'};
                auto fw = new TYTFW(magic);
                fw->Read(cmd["info"].as<std::string>());
                std::cerr << fw->ToString();
                exit(0);
            }

            if (!cmd.count("device"))
            {
                std::wcout << L"You must select a device" << std::endl;
                exit(1);
            }

            auto dev_idx = cmd["device"].as<uint16_t>();
            if (dfu->Open(dev_idx))
            {
                //std::cerr << "Model: " << dfu->IdentifyDevice() << std::endl;

                if(cmd.count("dump-reg")) 
                {
                    auto x = cmd["dump-reg"].as<uint16_t>();
                    //dump registers
                    //for (auto x = 0; x < 0xFF; x++)
                    //{
                        std::cerr << "Read register: 0x" << std::setfill('0') << std::setw(2) << std::hex << x << std::endl;
                        tyt_tool::PrintHex(dfu->ReadRegister(static_cast<const TYTRegister>(x)));
                    //}
                }
            }
            dfu->Close();
        }
    }
    catch (DFUException& e) 
    {
        std::cout << "Error: " << e.what() << std::endl;
    }
    catch (const cxxopts::OptionException &e)
    {
        std::cout << "error parsing options: " << e.what() << std::endl;
        exit(1);
    }
}

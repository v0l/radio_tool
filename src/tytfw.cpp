#include <tyt_dfu.hpp>

#include <iostream>
#include <filesystem>
#include <cxxopts.hpp>

using namespace tytfw::dfu;

auto constexpr VID = 0x0483;
auto constexpr PID = 0xdf11;

int main(int argc, char **argv)
{
    try
    {
        cxxopts::Options options("tytfw", " - TYT firmware tool");

        options.add_options("General")
        ("h,help", "Show this message")
        ("list", "List devices")
        ("d,device", "Device to use", cxxopts::value<uint16_t>(), "<index>");

        options.add_options("Programming")
            ("get-time", "Gets the radio time")
            ("set-time", "Sets the radio time")
            ("f,flash", "Flash firmware", cxxopts::value<std::string>(), "<firmware.bin>")
            ("p,program", "Upload codeplug", cxxopts::value<std::string>(), "<codeplug.rtd>")
        ;

        options.add_options("Firmware")
            ("wrap", "Wrap a firmware bin", cxxopts::value<std::string>(), "<firmware.bin>")
            ("unwrap", "Unwrap a fimrware file", cxxopts::value<std::string>(), "<firmware.bin>")
        ;

        auto cmd = options.parse(argc, argv);

        if (cmd.count("help") || cmd.arguments().empty())
        {
            std::cerr << options.help() << std::endl;
            exit(0);
        }

        auto dfu = new TYTDFU(VID, PID);
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

            if(!cmd.count("device")) {
                std::wcout << L"You must select a device" << std::endl;
                exit(1);
            }

            auto dev_idx = cmd["device"].as<uint16_t>();
            if(dfu->Open(dev_idx)) {
                dfu->EnterDFUMode();
                std::cerr << "Model: " << dfu->IdentifyDevice() << std::endl;
            }
        }
    }
    catch (const cxxopts::OptionException &e)
    {
        std::cout << "error parsing options: " << e.what() << std::endl;
        exit(1);
    }
}

#include <iostream>
#include <vector>
#include <exception>

#include <radio_tool/fw/fw_factory.hpp>
#include <radio_tool/util/flash.hpp>
#include <xor_tool.hpp>

using namespace radio_tool::fw;
using namespace radio_tool::flash;

int main(int argc, char **argv)
{
    if (argc != 2)
    {
        std::cerr << "Usage: " << argv[0] << " <firmware.bin>" << std::endl;
        exit(1);
    }
    const char *file = argv[1];

    std::cout << "Testing: " << file << std::endl;

    auto h = FirmwareFactory::GetFirmwareHandler(file);
    h->Read(file);

    std::cout << h->ToString();

    //test key
    auto key = XORTool::MakeXOR(h->GetData());
    for (const auto &region : h->GetDataSegments())
    {
        //Only test segments which are mapped to mcu flash section
        if (FlashUtil::GetSector(STM32F40X, region.address))
        {
            if (radio_tool::fw::XORTool::Verify(region.address, region.data, key))
            {
                std::cout
                    << "Region @ 0x" << std::setfill('0') << std::setw(8) << std::hex << region.address
                    << " appears to be a valid vector_table" << std::endl;
            }
            else
            {
                throw std::runtime_error("XOR appears to be wrong");
            }
        }
    }
    exit(0);
}
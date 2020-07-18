#include <iostream>

#include <radio_tool/fw/fw_factory.hpp>

int main(int, char** argv) {
    const char* file = argv[1];
    const char* radio = argv[2];

    std::cout << "Testing: " << file << " (" << radio << ")" << std::endl;

    auto fwF = radio_tool::fw::FirmwareFactory();

    auto h = fwF.GetFirmwareHandler(file);
    h->Read(file);

    std::cout << h->ToString();
    
    if(h->GetRadioModel() != std::string(radio)) {
        throw std::runtime_error("Firmware model incorrect");
    }
    return 0;
}
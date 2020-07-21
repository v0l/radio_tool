#include <iostream>

#include <radio_tool/fw/fw_factory.hpp>

int main(int argc, char** argv) {
    if (argc != 3) {
        std::cerr << "Usage: " << argv[0] << " <firmware.bin> <radio_model_check>" << std::endl;
        exit(1);
    }
    const char* file = argv[1];
    const char* radio = argv[2];

    std::cout << "Testing: " << file << " (" << radio << ")" << std::endl;

    auto h = radio_tool::fw::FirmwareFactory::GetFirmwareHandler(file);
    h->Read(file);

    std::cout << h->ToString();
    
    if(h->GetRadioModel() != std::string(radio)) {
        throw std::runtime_error("Firmware model incorrect");
    }
    exit(0);
}
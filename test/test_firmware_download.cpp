#include <iostream>

#include <radio_tool/fw/fw_factory.hpp>
#include "dummy_device.hpp"

int main(int argc, char** argv) {
    if (argc != 3) {
        std::cerr << "Usage: " << argv[0] << " <firmware.bin> <radio_model_check>" << std::endl;
        exit(1);
    }
    const char* file = argv[1];
    const char* radio = argv[2];

    std::cout << "Testing: " << file << " (" << radio << ")" << std::endl;

    
    exit(0);
}
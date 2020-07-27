#include <iostream>

#include <radio_tool/fw/fw_factory.hpp>

int main(int argc, char **argv)
{
    if (argc != 3)
    {
        std::cerr << "Usage: " << argv[0] << " <firmware.bin> <radio_model_check>" << std::endl;
        exit(1);
    }
    const char *file = argv[1];
    const char *radio = argv[2];

    std::cout << "Testing: " << file << " (" << radio << ")" << std::endl;

    auto h = radio_tool::fw::FirmwareFactory::GetFirmwareFileHandler(file);
    h->Read(file);

    std::cout << h->ToString();

    if (h->GetRadioModel() != std::string(radio))
    {
        std::cerr << "Firmware model incorrect" << std::endl;
        exit(1);
    }

    //test read/write
    auto write_test_name = "write_test.bin";
    h->Write(write_test_name);
    std::ifstream fa(file, std::ios_base::binary);
    std::ifstream fb(write_test_name, std::ios_base::binary);

    //check length fist
    fa.seekg(0, std::ios_base::end);
    fb.seekg(0, std::ios_base::end);
    auto 
        len_a = fa.tellg(),
        len_b = fb.tellg();
    
    if(len_a != len_b)
    {
        std::cerr << "Write test failed, file lengths dont match" << std::endl;
        exit(1);
    }
    
    fa.seekg(0, std::ios_base::beg);
    fb.seekg(0, std::ios_base::beg);
    while(true) 
    {
        if(fa.eof() || fb.eof()) break;

        uint8_t a, b;
        fa.read((char*)&a, 1);
        fb.read((char*)&b, 1);
        if(a != b)
        {
            auto pos = fa.tellg();
            if(pos >= 0x10 && pos <= 0x20 && ((a == 0x00 && b == 0xff) || (a == 0xff && b == 0x00)))
            {
                //if inside the header and some padding value is different
                //just continue, we wont write the header exactly the same
                //for the radio model
                continue;
            }
            if(pos >= 0x7c && pos <= 0x80 && a == 0xff)
            {
                //if original firmware didnt fill the n_regions just skip this
                continue;
            }
            std::stringstream ssmsg;
            ssmsg << "Write test failed, files are not the same @ 0x"
                << std::hex << pos;
            std::cerr << ssmsg.str() << std::endl;
            exit(1);
        }
    }

    std::cout << "Passed!" << std::endl;
    exit(0);
}
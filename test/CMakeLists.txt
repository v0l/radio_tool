cmake_minimum_required(VERSION 3.0.0)
project(radio_tool_tests VERSION 0.1.0)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()

set(CMAKE_CXX_FLAGS "-Wall -Wextra")
set(CMAKE_CXX_FLAGS_DEBUG "-g")
set(CMAKE_CXX_FLAGS_RELEASE "-Og")

include(ExternalData)
set(ExternalData_URL_TEMPLATES
    "https://v0l.io/radio_tool/firmware/%(algo)/%(hash)"
 )

#Test TYT Firmware parsing
add_executable(test_fw test_fw.cpp)
target_include_directories(test_fw PUBLIC ../include)
target_link_libraries(test_fw radiotool)

#Add firmware tests, "radio" is the model returned from GetRadioModel()
function(AddFirmwareTest file radio)
    ExternalData_Add_Test(data_${file}
        NAME test_fw_${file}
        COMMAND test_fw DATA{./firmware/${file}} ${radio}
    )
    ExternalData_Add_Target(data_${file})
endfunction()

#DM-1701
AddFirmwareTest(dm1701_2.03.bin "DM1701")

#MD-2017
AddFirmwareTest(md2017_csv_v4.15.bin "MD2017")
AddFirmwareTest(md2017_csv_gps_p4.17.bin "MD2017 GPS")
AddFirmwareTest(md2017_rec_gps_s4.17.bin "MD2017 GPS")
AddFirmwareTest(md2017_rec_d4.15.bin "MD2017")

#MD-UV380
AddFirmwareTest(uv380_csv_v18.11.bin "UV380")
AddFirmwareTest(uv380_rec_d18.11.bin "UV380")

#MD-UV390
AddFirmwareTest(uv390_csv_gps_p18.11.bin "UV390")
AddFirmwareTest(uv390_rec_gps_s18.11.bin "UV390")

#MD-9600
AddFirmwareTest(md9600_rec_gps_s6.13.bin "MD9600")
AddFirmwareTest(md9600_csv_gps_p6.13.bin "MD9600")
AddFirmwareTest(md9600_csv_v6.09.bin "MD9600")
AddFirmwareTest(md9600_rec_d6.09.bin "MD9600")

#MD-390
AddFirmwareTest(TYT-Vocoder-MD390-S15.04.bin "MD390")

#MD-380
AddFirmwareTest(TYT-Vocoder-MD380-D14.07.bin "MD380")
AddFirmwareTest(TYT-Vocoder-MD380-D13.20.bin "MD380")
AddFirmwareTest(TYT-TFT-MD380-D3.20.bin "MD380")

#MD-446
AddFirmwareTest(TYT-Vocoder-MD446-D13.20.bin "MD380")
AddFirmwareTest(TYT-TFT-MD446-D3.20.bin "MD380")

#MD-280
AddFirmwareTest(MD280.002.018.bin "MD280")
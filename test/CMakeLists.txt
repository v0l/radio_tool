cmake_minimum_required(VERSION 3.0.0)
project(radio_tool_tests VERSION 0.1.0)

include(ExternalData)
set(ExternalData_URL_TEMPLATES
    "https://data.v0l.io/radio_tool/firmware/%(algo)/%(hash)"
 )

if (${CMAKE_CXX_COMPILER_ID} STREQUAL GNU OR ${CMAKE_CXX_COMPILER_ID} STREQUAL AppleClang)
  link_libraries(PkgConfig::LibUSB)
  include_directories(${LibUSB_INCLUDEDIR})
elseif (${CMAKE_CXX_COMPILER_ID} STREQUAL MSVC)
  include_directories(${VCPKG_INCLUDE_DIR})
  link_directories(${VCPKG_LIB_DIR})
  link_libraries(libusb-1.0)
endif()

include_directories(../include)
link_libraries(radiotool)

if(XOR_TOOL_INC)
  add_executable(test_xor test_xor_tool.cpp)
  include_directories(${XOR_TOOL_INC})
  add_definitions(-DXOR_TOOL)
endif()

add_executable(test_fw test_fw.cpp)
add_executable(test_util test_util.cpp)

#Add firmware tests, "radio" is the model returned from GetRadioModel()
function(AddFirmwareTest file radio)
    ExternalData_Add_Test(data_${file}
        NAME test_fw_${file}
        COMMAND test_fw DATA{./firmware/${file}} ${radio}
    )
    if(XOR_TOOL_INC)
      ExternalData_Add_Test(data_${file}
          NAME test_xor_${file}
          COMMAND test_xor DATA{./firmware/${file}}
      )
    endif()
    ExternalData_Add_Target(data_${file})
endfunction()

# Firmware tests disabled - firmware files no longer available
#DM-1701
#AddFirmwareTest(dm1701_2.03.bin "DM1701")

#MD-2017
#AddFirmwareTest(TYT2017_UV_CSV__V3.33.bin "MD2017")
#AddFirmwareTest(TYT2017_UV_CSV__V3.35.bin "MD2017")
#AddFirmwareTest(TYT2017_UV_CSV__V3.36.bin "MD2017")
#AddFirmwareTest(TYT2017_UV_CSV_GPS__P3.33.bin "MD2017 GPS")
#AddFirmwareTest(TYT2017_UV_CSV_GPS__P3.35.bin "MD2017 GPS")
#AddFirmwareTest(TYT2017_UV_CSV_GPS__P3.38.bin "MD2017 GPS")
#AddFirmwareTest(TYT2017_UV_GPS_REC__S3.31.bin "MD2017 GPS")
#AddFirmwareTest(TYT2017_UV_GPS_REC__S3.38.bin "MD2017 GPS")
#AddFirmwareTest(TYT2017_UV_REC__D3.31.bin "MD2017")
#AddFirmwareTest(TYT2017_UV_REC__D3.36.bin "MD2017")
#AddFirmwareTest(TYT2017_UVCSV_GPS_P3.35.bin "MD2017 GPS")
#AddFirmwareTest(TYT2017_UVCSV_V3.35.bin "MD2017")
#AddFirmwareTest(TYT2017_UVGPS_REC_S3.31.bin "MD2017 GPS")
#AddFirmwareTest(TYT2017_UVGPS_REC_S3.38.bin "MD2017 GPS")
#AddFirmwareTest(TYT2017_UVREC_D3.31.bin "MD2017")
#AddFirmwareTest(TYT2017_UVREC_D3.36.bin "MD2017")
#AddFirmwareTest(md2017_csv_v4.15.bin "MD2017")
#AddFirmwareTest(md2017_csv_gps_p4.17.bin "MD2017 GPS")
#AddFirmwareTest(md2017_rec_gps_s4.17.bin "MD2017 GPS")
#AddFirmwareTest(md2017_rec_d4.15.bin "MD2017")

#MD-UV380
#AddFirmwareTest(uv380_csv_v18.11.bin "UV3X0")
#AddFirmwareTest(uv380_rec_d18.11.bin "UV3X0")

#MD-UV390
#AddFirmwareTest(uv390_csv_gps_p18.11.bin "UV3X0 GPS")
#AddFirmwareTest(uv390_rec_gps_s18.11.bin "UV3X0 GPS")

#MD-9600
#AddFirmwareTest(md9600_rec_gps_s6.13.bin "MD9600")
#AddFirmwareTest(md9600_csv_gps_p6.13.bin "MD9600")
#AddFirmwareTest(md9600_csv_v6.09.bin "MD9600")
#AddFirmwareTest(md9600_rec_d6.09.bin "MD9600")

#MD-390
#AddFirmwareTest(TYT_Vocoder_MD390_S13.12.bin "MD390")
#AddFirmwareTest(TYT_Vocoder_MD390_S13.20.bin "MD390")
#AddFirmwareTest(TYT_Vocoder_MD390_S15.02.bin "MD390")
#AddFirmwareTest(TYT_Vocoder_MD390_S15.04.bin "MD390")

#MD-380
#AddFirmwareTest(TYT_Vocoder_MD380_D13.14.bin "MD380")
#AddFirmwareTest(TYT_Vocoder_MD380_D13.20.bin "MD380")
#AddFirmwareTest(TYT_Vocoder_MD380_D14.07.bin "MD380")
#AddFirmwareTest(TYT_TFT_MD380_D3.20.bin "MD380")
#AddFirmwareTest(D013.034.bin "MD380")

#MD-446
#AddFirmwareTest(TYT_Vocoder_MD446_D13.20.bin "MD380")
#AddFirmwareTest(TYT_TFT_MD446_D3.20.bin "MD380")

#MD-280
#AddFirmwareTest(MD280.002.018.bin "MD280")

#RT3S
#AddFirmwareTest(RT3S_CSV__V16.06.bin "UV3X0")
#AddFirmwareTest(RT3S_CSV_GPS__P16.06.bin "UV3X0 GPS")
#AddFirmwareTest(RT3S_GPS_REC__S16.06.bin "UV3X0 GPS")
#AddFirmwareTest(RT3S_REC__D16.06.bin "UV3X0")

#CS800
#AddFirmwareTest(CS800D_Host_MS.30.14_Use_Flashburn3.0_Upgrade.bin "CS800")
#AddFirmwareTest(TEST_CS800_4000__Mobile_M100000_Host_S3.02.07_Auth_Test_Ver_1_.bin "CS800")

#GD-77
#AddFirmwareTest(GD-77_V3.2.1.sgl "GD77")
#AddFirmwareTest(GD-77_V3.2.2.sgl "GD77")
#AddFirmwareTest(GD-77_V4.2.6.sgl "GD77")
#AddFirmwareTest(GD-77_V4.2.8.sgl "GD77")

#GD-77S
#AddFirmwareTest(GD-77S_V1.2.0.sgl "GD77S")
#AddFirmwareTest(GD-77S_V1.3.0.sgl "GD77S")

#BF-5R
#AddFirmwareTest(BF-5R_V2.1.0.sgl "BF5R")
#AddFirmwareTest(BF-5R_V2.1.6.sgl "BF5R")
#AddFirmwareTest(RD-5R_V2.0.9.sgl "BF5R")
cmake_minimum_required(VERSION 3.0.0)
project(radio_tool VERSION 0.1.0)

include(CTest)
enable_testing()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(CMAKE_CXX_FLAGS_DEBUG_INIT "-Wall")
set(CMAKE_CXX_FLAGS_RELEASE_INIT "-Wall")

set(ALL_SRC 
    src/radio_tool.cpp
    src/dfu.cpp
    src/radio.cpp
    src/fw_factory.cpp
    src/radio_factory.cpp
    src/tyt_radio.cpp
    src/tyt_dfu.cpp
    src/tyt_fw.cpp
)
add_executable(radio_tool src/radio_tool.cpp ${ALL_SRC})
target_include_directories(radio_tool PUBLIC include)
target_link_libraries(radio_tool usb-1.0)

set(CPACK_PROJECT_NAME ${PROJECT_NAME})
set(CPACK_PROJECT_VERSION ${PROJECT_VERSION})
include(CPack)

cmake_minimum_required(VERSION 3.0.0)
project(radio_tool VERSION 0.1.0)

option(BUILD_TESTING "Enable tests" OFF)
if(BUILD_TESTING)
  include(CTest)
  enable_testing()
  add_subdirectory(test)
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()

set(CMAKE_CXX_FLAGS "-Wall -Wextra")
set(CMAKE_CXX_FLAGS_DEBUG "-g")
set(CMAKE_CXX_FLAGS_RELEASE "-O3")

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

add_library(radiotool ${ALL_SRC})
target_include_directories(radiotool PUBLIC include)
target_link_libraries(radiotool usb-1.0)

add_executable(radio_tool src/radio_tool.cpp)
target_include_directories(radio_tool PUBLIC include)
target_link_libraries(radio_tool usb-1.0)
target_link_libraries(radio_tool radiotool)

set(CPACK_PROJECT_NAME ${PROJECT_NAME})
set(CPACK_PROJECT_VERSION ${PROJECT_VERSION})
set(CPACK_PACKAGE_CHECKSUM SHA256)
include(CPack)

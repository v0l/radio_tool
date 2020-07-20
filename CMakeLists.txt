cmake_minimum_required(VERSION 3.0.0)
project(radio_tool VERSION 0.1.0)

option(BUILD_TESTING "Enable tests" OFF)

if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()
string(TOLOWER ${CMAKE_BUILD_TYPE} CMAKE_BUILD_TYPE_LOWER)

if(NOT CMAKE_GENERATOR_PLATFORM)
  set(CMAKE_GENERATOR_PLATFORM x64)
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_LIST_DIR}/cmake")
if (${CMAKE_CXX_COMPILER_ID} STREQUAL GNU OR ${CMAKE_CXX_COMPILER_ID} STREQUAL AppleClang)
  find_package(LibUSB REQUIRED)
  set(CMAKE_CXX_FLAGS "-Wall -Wextra")
  set(CMAKE_CXX_FLAGS_DEBUG "-g")
  set(CMAKE_CXX_FLAGS_RELEASE "-O3")
elseif (${CMAKE_CXX_COMPILER_ID} STREQUAL MSVC)
  set(VCPKG_INCLUDE_DIR $ENV{VCPKG_ROOT}\\installed\\${CMAKE_GENERATOR_PLATFORM}-windows\\include)
  if(${CMAKE_BUILD_TYPE_LOWER} STREQUAL release)
	set(VCPKG_LIB_DIR $ENV{VCPKG_ROOT}\\installed\\${CMAKE_GENERATOR_PLATFORM}-windows\\lib)
  elseif (${CMAKE_BUILD_TYPE_LOWER} STREQUAL debug)
    set(VCPKG_LIB_DIR $ENV{VCPKG_ROOT}\\installed\\${CMAKE_GENERATOR_PLATFORM}-windows\\debug\\lib)
  endif()
  message("-- VCPKG include dir: ${VCPKG_INCLUDE_DIR}")
  message("-- VCPKG lib dir: ${VCPKG_LIB_DIR}")
endif()

if(BUILD_TESTING)
  include(CTest)
  enable_testing()
  add_subdirectory(test)
endif()

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

add_executable(radio_tool src/radio_tool.cpp)
target_include_directories(radio_tool PUBLIC include)
target_link_libraries(radio_tool radiotool)

if (${CMAKE_CXX_COMPILER_ID} STREQUAL GNU OR ${CMAKE_CXX_COMPILER_ID} STREQUAL AppleClang)
  target_link_libraries(radiotool PkgConfig::LibUSB)
  target_link_libraries(radio_tool PkgConfig::LibUSB)
  target_include_directories(radiotool PUBLIC ${LibUSB_INCLUDEDIR})
  target_include_directories(radio_tool PUBLIC ${LibUSB_INCLUDEDIR})
elseif (${CMAKE_CXX_COMPILER_ID} STREQUAL MSVC)
  target_include_directories(radiotool PUBLIC ${VCPKG_INCLUDE_DIR})
  target_link_directories(radiotool PRIVATE ${VCPKG_LIB_DIR})
  target_include_directories(radio_tool PUBLIC ${VCPKG_INCLUDE_DIR})
  target_link_directories(radio_tool PRIVATE ${VCPKG_LIB_DIR})
  
  target_link_libraries(radiotool libusb-1.0)
  target_link_libraries(radio_tool libusb-1.0)
endif()

install(TARGETS radio_tool)

set(CPACK_PROJECT_NAME ${PROJECT_NAME})
set(CPACK_PROJECT_VERSION ${PROJECT_VERSION})
set(CPACK_PACKAGE_DESCRIPTION "Universal radio firmware tool")
set(CPACK_PACKAGE_HOMEPAGE_URL "https://github.com/v0l/radio_tool")
set(CPACK_PACKAGE_CONTACT "Kieran Harkin <kieran+git@harkin.me>")
set(CPACK_RESOURCE_FILE_LICENSE "${PROJECT_SOURCE_DIR}/LICENSE")
set(CPACK_GENERATOR "DEB" "TGZ" "TXZ")
set(CPACK_PACKAGE_CHECKSUM SHA256)
set(CPACK_STRIP_FILES true)

set(CPACK_DEBIAN_PACKAGE_ARCHITECTURE "amd64")
set(CPACK_DEBIAN_PACKAGE_DEPENDS "libusb-1.0-0")
set(CPACK_DEBIAN_PACKAGE_SECTION "hamradio")
set(CPACK_DEBIAN_COMPRESSION_TYPE "xz")
include(CPack)
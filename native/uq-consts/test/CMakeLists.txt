# Builds and runs the C++ and QML uq-consts tests: cmake -S native/uq-consts/test -B build && ctest --test-dir build
cmake_minimum_required(VERSION 3.21)
project(uqconsts_tests LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_AUTOMOC ON)
find_package(Qt6 REQUIRED COMPONENTS Core Test)
enable_testing()
add_executable(tst_uqconsts tst_uqconsts.cpp ../uqconsts.h)
target_compile_options(tst_uqconsts PRIVATE -Wall -Wextra -Werror)
target_link_libraries(tst_uqconsts PRIVATE Qt6::Core Qt6::Test)
add_test(NAME tst_uqconsts COMMAND tst_uqconsts)
# The QML theme base, with qmltestrunner (qt6-declarative) when it is installed.
find_program(QMLTESTRUNNER qmltestrunner PATHS /usr/lib/qt6/bin)
if(QMLTESTRUNNER)
    add_test(NAME tst_uqthemebase COMMAND ${QMLTESTRUNNER} -import ${CMAKE_CURRENT_SOURCE_DIR}/qml
             -input ${CMAKE_CURRENT_SOURCE_DIR}/qml/tst_uqthemebase.qml)
endif()

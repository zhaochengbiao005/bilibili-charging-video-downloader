import sys, os
from PyQt6.QtWidgets import QApplication
from PyQt6.QtGui import QFont
from gui_qt.window import MainWindow

QSS_PATH = os.path.join(os.path.dirname(__file__), 'resources', 'styles.qss')

def run():
    app = QApplication(sys.argv)
    app.setFont(QFont("Microsoft YaHei UI", 10))

    if os.path.exists(QSS_PATH):
        with open(QSS_PATH, encoding='utf-8') as f:
            app.setStyleSheet(f.read())

    window = MainWindow()
    window.show()
    sys.exit(app.exec())

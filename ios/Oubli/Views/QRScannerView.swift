import SwiftUI
import AVFoundation

/// Camera-based QR code scanner wrapped for SwiftUI.
struct QRScannerView: UIViewControllerRepresentable {
    let onCodeScanned: (String) -> Void

    func makeUIViewController(context: Context) -> ScannerViewController {
        let vc = ScannerViewController()
        vc.onCodeScanned = onCodeScanned
        return vc
    }

    func updateUIViewController(_ uiViewController: ScannerViewController, context: Context) {}

    class ScannerViewController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
        var onCodeScanned: ((String) -> Void)?
        private var captureSession: AVCaptureSession?
        private var hasScanned = false

        override func viewDidLoad() {
            super.viewDidLoad()
            view.backgroundColor = .black

            let session = AVCaptureSession()
            guard let device = AVCaptureDevice.default(for: .video),
                  let input = try? AVCaptureDeviceInput(device: device),
                  session.canAddInput(input) else {
                showFallback()
                return
            }

            session.addInput(input)

            let output = AVCaptureMetadataOutput()
            guard session.canAddOutput(output) else {
                showFallback()
                return
            }
            session.addOutput(output)
            output.setMetadataObjectsDelegate(self, queue: .main)
            output.metadataObjectTypes = [.qr]

            let preview = AVCaptureVideoPreviewLayer(session: session)
            preview.frame = view.bounds
            preview.videoGravity = .resizeAspectFill
            view.layer.addSublayer(preview)

            captureSession = session

            // Scanner overlay
            let overlayView = ScannerOverlayView()
            overlayView.frame = view.bounds
            overlayView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            overlayView.backgroundColor = .clear
            view.addSubview(overlayView)

            // Hint label
            let hintLabel = UILabel()
            hintLabel.text = "Point camera at QR code"
            hintLabel.textColor = .white
            hintLabel.font = .systemFont(ofSize: 15, weight: .medium)
            hintLabel.textAlignment = .center
            hintLabel.translatesAutoresizingMaskIntoConstraints = false
            view.addSubview(hintLabel)
            NSLayoutConstraint.activate([
                hintLabel.centerXAnchor.constraint(equalTo: view.centerXAnchor),
                hintLabel.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor, constant: -80),
            ])

            DispatchQueue.global(qos: .userInitiated).async {
                session.startRunning()
            }
        }

        override func viewDidLayoutSubviews() {
            super.viewDidLayoutSubviews()
            if let preview = view.layer.sublayers?.first as? AVCaptureVideoPreviewLayer {
                preview.frame = view.bounds
            }
        }

        override func viewWillDisappear(_ animated: Bool) {
            super.viewWillDisappear(animated)
            captureSession?.stopRunning()
        }

        func metadataOutput(_ output: AVCaptureMetadataOutput,
                            didOutput metadataObjects: [AVMetadataObject],
                            from connection: AVCaptureConnection) {
            guard !hasScanned,
                  let object = metadataObjects.first as? AVMetadataMachineReadableCodeObject,
                  let value = object.stringValue else { return }
            hasScanned = true
            captureSession?.stopRunning()
            onCodeScanned?(value)
        }

        // MARK: - Scanner Overlay

        private class ScannerOverlayView: UIView {
            override func draw(_ rect: CGRect) {
                guard let ctx = UIGraphicsGetCurrentContext() else { return }
                let cutoutSize = min(rect.width, rect.height) * 0.65
                let cutoutRect = CGRect(
                    x: (rect.width - cutoutSize) / 2,
                    y: (rect.height - cutoutSize) / 2,
                    width: cutoutSize,
                    height: cutoutSize
                )
                // Semi-transparent background
                ctx.setFillColor(UIColor.black.withAlphaComponent(0.5).cgColor)
                ctx.fill(rect)
                // Clear cutout
                ctx.setBlendMode(.clear)
                let cutoutPath = UIBezierPath(roundedRect: cutoutRect, cornerRadius: 12)
                ctx.addPath(cutoutPath.cgPath)
                ctx.fillPath()
                // White border
                ctx.setBlendMode(.normal)
                ctx.setStrokeColor(UIColor.white.cgColor)
                ctx.setLineWidth(2)
                ctx.addPath(cutoutPath.cgPath)
                ctx.strokePath()
            }
        }

        private func showFallback() {
            let label = UILabel()
            label.text = "Camera not available"
            label.textColor = .white
            label.textAlignment = .center
            label.frame = view.bounds
            label.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            view.addSubview(label)
        }
    }
}

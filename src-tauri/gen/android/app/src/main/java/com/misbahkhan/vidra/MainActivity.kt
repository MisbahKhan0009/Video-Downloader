package com.misbahkhan.vidra

import android.content.Intent
import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import java.io.File

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    handleShare(intent)
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    handleShare(intent)
  }

  /**
   * A link shared from another app is parked in a small file; the Rust side
   * (`take_shared_link`) picks it up and the UI grabs it automatically.
   */
  private fun handleShare(intent: Intent?) {
    if (intent?.action != Intent.ACTION_SEND) return
    val text = intent.getStringExtra(Intent.EXTRA_TEXT) ?: return
    try {
      File(filesDir, "shared_link.txt").writeText(text)
    } catch (_: Exception) {
    }
  }
}

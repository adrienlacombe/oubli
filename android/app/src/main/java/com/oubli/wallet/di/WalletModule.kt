package com.oubli.wallet.di

import android.content.Context
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

/**
 * Hilt module providing application-scoped dependencies.
 *
 * The [uniffi.oubli.OubliWallet] instance is NOT provided here because it requires
 * a [FragmentActivity] reference for biometric prompts. It is created lazily in the
 * ViewModel once an Activity is attached.
 */
@Module
@InstallIn(SingletonComponent::class)
object WalletModule {

    @Provides
    @Singleton
    fun provideApplicationContext(@ApplicationContext context: Context): Context = context
}
